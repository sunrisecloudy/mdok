#!/usr/bin/env python3
"""Postman JS corpus -> mdok-pm-probe coverage gate (spec QUICKJS_PROBE_SPEC.md §9-10).

Reads the Postman corpus produced by scripts/fetch_postman_corpus.py
(tests/corpus/postman-js/corpus.json + collections/*.json), walks every
`item` tree collecting test/prerequest event scripts, runs each script
through the `mdok-pm-probe` QuickJS probe as an isolated case, aggregates
used_api/diagnostics/outcomes, and enforces the coverage gate:

    uncovered = used - supported  must be empty
    exit 0  <=>  corpus non-empty AND uncovered empty (and >=1 script ran)
    else exit 1

Reports are written to --out/report.json and --out/report.md.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any

# ---------------------------------------------------------------------------
# Constants (probe contract)
# ---------------------------------------------------------------------------

DEFAULT_PROBE = "target/release/mdok-pm-probe"
FALLBACK_PROBE = ["cargo", "run", "-p", "mdok-quickjs", "--bin", "mdok-pm-probe", "--"]
SCRIPT_TIMEOUT_MS = 2000
API_VERSION = "postman-cli-v1"
CANNED_RESPONSE = {
    "code": 200,
    "status": "OK",
    "headers": [],
    "body": '{"ok":true}',
}
PROBE_PROCESS_TIMEOUT_S = 120  # hard wall-clock guard around each probe run
LISTEN_PHASES = {"test": "test", "prerequest": "prerequest"}
MAX_WORKERS = 8

# Port of crates/mdok-postman/src/lib.rs `looks_secret` (same heuristic).
_SECRET_MARKERS = (
    "secret",
    "password",
    "passwd",
    "token",
    "api_key",
    "apikey",
    "authorization",
    "cookie",
    "set_cookie",
    "credential",
    "private_key",
    "client_secret",
)


def looks_secret(name: str) -> bool:
    normalized = name.lower().replace("-", "_").replace(".", "_")
    return any(marker in normalized for marker in _SECRET_MARKERS)


# ---------------------------------------------------------------------------
# Collection parsing
# ---------------------------------------------------------------------------


class CorpusError(Exception):
    """Harness-level corpus problem (missing manifest, bad layout...)."""


def load_manifest(corpus_dir: Path) -> list[dict[str, Any]]:
    manifest_path = corpus_dir / "corpus.json"
    if not manifest_path.is_file():
        raise CorpusError(f"corpus manifest not found: {manifest_path}")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CorpusError(f"corpus manifest is not valid JSON: {manifest_path}: {exc}") from exc
    entries = manifest.get("entries", [])
    if not isinstance(entries, list):
        raise CorpusError(f"corpus manifest has no entries list: {manifest_path}")
    return entries


def collection_file_for(entry: dict[str, Any], corpus_dir: Path) -> Path:
    """Locate the collection file for a manifest entry."""
    index = entry.get("index")
    name = entry.get("name", "collection")
    candidates = [
        corpus_dir / "collections" / f"{index}-{name}.json",
    ]
    if "collection_path" in entry:
        candidates.insert(0, corpus_dir / str(entry["collection_path"]))
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    # Last resort: any file in collections/ starting with "<index>-" (the
    # corpus downloader zero-pads indexes, e.g. 0066-*.json).
    if index is not None:
        for padded in (f"{index}", f"{int(index):04d}"):
            for candidate in sorted((corpus_dir / "collections").glob(f"{padded}-*.json")):
                return candidate
    return candidates[0]


def header_list(value: Any) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    if not isinstance(value, list):
        return out
    for header in value:
        if not isinstance(header, dict) or header.get("disabled") is True:
            continue
        key = header.get("key")
        if key is None:
            continue
        raw_value = header.get("value")
        out.append({"key": str(key), "value": "" if raw_value is None else str(raw_value)})
    return out


def url_raw(url: Any) -> str:
    if isinstance(url, str):
        return url
    if not isinstance(url, dict):
        return ""
    raw = url.get("raw")
    if isinstance(raw, str) and raw:
        return raw
    protocol = str(url.get("protocol") or "https")
    host = url.get("host") or []
    if isinstance(host, str):
        host = [host]
    port = url.get("port")
    path = url.get("path") or []
    if isinstance(path, str):
        path = [path]
    query = url.get("query") or []
    base = f"{protocol}://" + ".".join(str(part) for part in host)
    if port:
        base += f":{port}"
    joined_path = "/".join(str(part) for part in path)
    result = base + (f"/{joined_path}" if joined_path else "")
    pairs = []
    for item in query:
        if isinstance(item, dict) and item.get("key") is not None:
            pairs.append(f"{item['key']}={item.get('value', '')}")
    if pairs:
        result += "?" + "&".join(pairs)
    return result


def request_body(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        return None
    mode = value.get("mode")
    raw = value.get("raw")
    if mode == "raw" and isinstance(raw, str) and raw:
        return {"mode": "raw", "raw": raw}
    if mode in ("urlencoded", "formdata"):
        parts = value.get(mode)
        if isinstance(parts, list):
            pairs = []
            for part in parts:
                if not isinstance(part, dict) or part.get("disabled") is True:
                    continue
                key = part.get("key")
                if key is None:
                    continue
                pairs.append(f"{key}={part.get('value', '')}")
            if pairs:
                return {"mode": mode, "raw": "&".join(pairs)}
    if isinstance(raw, str) and raw:
        return {"mode": mode or "raw", "raw": raw}
    return None


def response_from_example(example: Any) -> dict[str, Any] | None:
    if not isinstance(example, dict):
        return None
    out: dict[str, Any] = {}
    if example.get("code") is not None:
        out["code"] = example["code"]
    if example.get("status") is not None:
        out["status"] = str(example["status"])
    headers = header_list(example.get("header"))
    if headers:
        out["headers"] = headers
    body = example.get("body")
    if isinstance(body, str) and body:
        out["body"] = body
    if example.get("responseTime") is not None:
        out["response_time_ms"] = example["responseTime"]
    if example.get("responseSize") is not None:
        out["response_size_bytes"] = example["responseSize"]
    return out or None


def variable_map(variables: Any) -> dict[str, str]:
    out: dict[str, str] = {}
    if not isinstance(variables, list):
        return out
    for variable in variables:
        if not isinstance(variable, dict):
            continue
        key = variable.get("key")
        if key is None:
            continue
        value = variable.get("value")
        out[str(key)] = "" if value is None else str(value)
    return out


def merge_variables(base: dict[str, str], extra: dict[str, str]) -> dict[str, str]:
    merged = dict(base)
    merged.update(extra)
    return merged


def walk_items(items: Any, folder_path: list[str], variables: dict[str, str]) -> list[dict[str, Any]]:
    """Recursively walk item trees, yielding script records.

    Each record: {collection_index, collection_name, folder_path, item_name,
    item_type, listen, phase, event_index, script}.
    """
    scripts: list[dict[str, Any]] = []
    if not isinstance(items, list):
        return scripts
    for item in items:
        if not isinstance(item, dict):
            continue
        item_name = str(item.get("name") or "unnamed")
        # Folder-level variables shadow collection variables for nested scripts.
        folder_vars = merge_variables(variables, variable_map(item.get("variable")))
        nested = item.get("item")
        if isinstance(nested, list):
            # A folder: collect its own events too, then recurse.
            scripts.extend(
                collect_events(
                    item,
                    folder_path=folder_path,
                    item_name=item_name,
                    item_type="folder",
                    variables=folder_vars,
                )
            )
            scripts.extend(
                walk_items(nested, folder_path + [item_name], folder_vars)
            )
        else:
            scripts.extend(
                collect_events(
                    item,
                    folder_path=folder_path,
                    item_name=item_name,
                    item_type="request",
                    variables=folder_vars,
                )
            )
    return scripts


def collect_events(
    item: dict[str, Any],
    *,
    folder_path: list[str],
    item_name: str,
    item_type: str,
    variables: dict[str, str],
) -> list[dict[str, Any]]:
    scripts: list[dict[str, Any]] = []
    events = item.get("event")
    if not isinstance(events, list):
        return scripts
    for event_index, event in enumerate(events):
        if not isinstance(event, dict):
            continue
        listen = event.get("listen")
        phase = LISTEN_PHASES.get(listen)  # type: ignore[arg-type]
        if phase is None:
            continue  # only test | prerequest participate in coverage
        script = script_source(event)
        scripts.append(
            {
                "folder_path": list(folder_path),
                "item_name": item_name,
                "item_type": item_type,
                "listen": listen,
                "phase": phase,
                "event_index": event_index,
                "script": script,
                "variables": dict(variables),
                "item": item,
            }
        )
    return scripts


def script_source(event: dict[str, Any]) -> str:
    script = event.get("script")
    if not isinstance(script, dict):
        return ""
    exec_value = script.get("exec")
    if isinstance(exec_value, str):
        return exec_value
    if isinstance(exec_value, list):
        parts = [str(part) for part in exec_value if part is not None]
        return "\n".join(parts)
    return ""


# ---------------------------------------------------------------------------
# Probe case construction (spec §9.2)
# ---------------------------------------------------------------------------


def build_case(script_record: dict[str, Any], collection_vars: dict[str, str]) -> dict[str, Any]:
    item = script_record["item"]
    request_obj = item.get("request") if isinstance(item, dict) else None

    request: dict[str, Any] = {
        "name": script_record["item_name"],
        "method": "GET",
        "url": "",
        "headers": [],
        "body": None,
    }
    if isinstance(request_obj, dict):
        method = request_obj.get("method")
        if isinstance(method, str) and method:
            request["method"] = method
        request["url"] = url_raw(request_obj.get("url"))
        request["headers"] = header_list(request_obj.get("header"))
        request["body"] = request_body(request_obj.get("body"))
    elif script_record["item_type"] == "folder":
        request["name"] = script_record["item_name"] + " (folder event)"

    response = CANNED_RESPONSE
    responses = item.get("response") if isinstance(item, dict) else None
    if isinstance(responses, list) and responses:
        example = response_from_example(responses[0])
        if example:
            response = example

    # Variables: collection scope seeded from the collection-level variable
    # array plus folder-level arrays along the item's folder chain. Names that
    # look secret (mdok-postman `looks_secret` heuristic) are additionally
    # listed in `secrets` so the probe taints/redacts their values.
    variables = script_record["variables"]
    secrets: list[str] = [name for name in variables if looks_secret(name)]
    for header in request["headers"]:
        if looks_secret(header["key"]) and header["key"] not in secrets:
            secrets.append(header["key"])

    return {
        "script": script_record["script"],
        "phase": script_record["phase"],
        "request": request,
        "response": response,
        "variables": {
            "global": {},
            "collection": dict(variables),
            "environment": {},
            "data": {},
            "local": {},
        },
        "secrets": secrets,
        "profile": {
            "api_version": API_VERSION,
            "script_timeout_ms": SCRIPT_TIMEOUT_MS,
        },
        "coverage": True,
    }


# ---------------------------------------------------------------------------
# Probe invocation
# ---------------------------------------------------------------------------


class ProbeUnavailable(Exception):
    """The mdok-pm-probe binary could not be resolved/run."""


class Probe:
    """Wraps the probe command prefix (path or `cargo run ... --`)."""

    def __init__(self, prefix: list[str], cwd: Path, description: str):
        self.prefix = prefix
        self.cwd = cwd
        self.description = description

    def _run(self, argv: list[str], input_bytes: bytes | None = None) -> subprocess.CompletedProcess:
        try:
            return subprocess.run(
                self.prefix + argv,
                input=input_bytes,
                capture_output=True,
                cwd=self.cwd,
                timeout=PROBE_PROCESS_TIMEOUT_S,
            )
        except subprocess.TimeoutExpired as exc:
            raise ProbeUnavailable(
                f"probe {self.description} timed out after {PROBE_PROCESS_TIMEOUT_S}s "
                f"on argv {argv!r}"
            ) from exc
        except OSError as exc:
            raise ProbeUnavailable(f"probe {self.description} failed to start: {exc}") from exc

    def list_api(self) -> dict[str, Any]:
        proc = self._run(["--list-api"])
        if proc.returncode != 0:
            raise ProbeUnavailable(
                f"probe {self.description} `--list-api` exited {proc.returncode}: "
                f"{proc.stdout[-1000:]}{proc.stderr[-2000:]}"
            )
        try:
            data = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            raise ProbeUnavailable(
                f"probe {self.description} `--list-api` returned non-JSON output: {exc}"
            ) from exc
        supported = data.get("supported")
        if not isinstance(supported, list):
            raise ProbeUnavailable(
                f"probe {self.description} `--list-api` has no `supported` list: {data!r}"
            )
        return data

    def run_case(self, case: dict[str, Any]) -> dict[str, Any]:
        payload = json.dumps(case, ensure_ascii=False).encode("utf-8")
        proc = self._run(["--case", "-", "--network", "offline"], input_bytes=payload)
        if not proc.stdout.strip():
            return {
                "ok": False,
                "outcome": "error",
                "duration_ms": None,
                "used_api": [],
                "diagnostics": [],
                "transcript": None,
                "probe_error": (
                    f"probe exited {proc.returncode} with no stdout: "
                    f"{proc.stderr[-2000:]}"
                ),
            }
        try:
            data = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            return {
                "ok": False,
                "outcome": "error",
                "duration_ms": None,
                "used_api": [],
                "diagnostics": [],
                "transcript": None,
                "probe_error": f"unparseable probe output ({exc}): {proc.stdout[-2000:]}",
            }
        if not isinstance(data, dict):
            return {
                "ok": False,
                "outcome": "error",
                "duration_ms": None,
                "used_api": [],
                "diagnostics": [],
                "transcript": None,
                "probe_error": f"probe output is not an object: {data!r}",
            }
        # Normalize: a probe harness error (ok:false) is tolerated per-script;
        # whatever API usage it recorded still counts toward coverage.
        for field in ("used_api", "diagnostics"):
            if not isinstance(data.get(field), list):
                data[field] = []
        if not isinstance(data.get("outcome"), str):
            data["outcome"] = "error"
        return data


def resolve_probe(probe_arg: str | None, repo_root: Path) -> Probe:
    if probe_arg:
        candidate = Path(probe_arg).expanduser()
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return Probe([str(candidate)], repo_root, str(candidate))
        if candidate.is_file():
            # Accept non-executable scripts (e.g. a Python stand-in probe):
            # run them through the current interpreter.
            head = candidate.read_bytes()[:256].decode("utf-8", "replace")
            if candidate.suffix == ".py" or "python" in head:
                return Probe([sys.executable, str(candidate)], repo_root, str(candidate))
        if shutil.which(probe_arg):
            return Probe([probe_arg], repo_root, probe_arg)
        raise ProbeUnavailable(f"--probe {probe_arg!r} is not an executable file")
    default = repo_root / DEFAULT_PROBE
    if default.is_file() and os.access(default, os.X_OK):
        return Probe([str(default)], repo_root, str(default))
    return Probe(FALLBACK_PROBE, repo_root, "cargo run -p mdok-quickjs --bin mdok-pm-probe")


# ---------------------------------------------------------------------------
# Collection processing (one worker task per collection)
# ---------------------------------------------------------------------------


def process_collection(
    entry: dict[str, Any], corpus_dir: Path, probe: Probe
) -> dict[str, Any]:
    index = entry.get("index")
    name = str(entry.get("name") or "collection")
    collection_path = collection_file_for(entry, corpus_dir)
    stats: dict[str, Any] = {
        "index": index,
        "name": name,
        "source_url": entry.get("source_url"),
        "collection_path": str(collection_path.relative_to(corpus_dir.parent.parent))
        if collection_path.is_relative_to(corpus_dir.parent.parent)
        else str(collection_path),
        "loaded": False,
        "load_error": None,
        "script_count": 0,
        "scripts_run": 0,
        "outcomes": {},
        "used_api": {},
        "diagnostics": [],
        "transcript": {
            "tests": 0,
            "tests_passed": 0,
            "tests_failed": 0,
            "logs": 0,
            "errors": 0,
            "child_requests": 0,
            "scope_writes": 0,
        },
        "duration_ms_total": 0,
    }
    try:
        collection = json.loads(collection_path.read_text(encoding="utf-8"))
    except OSError as exc:
        stats["load_error"] = f"cannot read {collection_path}: {exc}"
        return stats
    except json.JSONDecodeError as exc:
        stats["load_error"] = f"invalid JSON in {collection_path}: {exc}"
        return stats

    collection_vars = variable_map(collection.get("variable"))
    scripts = walk_items(collection.get("item"), [], collection_vars)
    stats["loaded"] = True
    stats["script_count"] = len(scripts)
    if not scripts:
        return stats

    per_script: list[dict[str, Any]] = []
    for record in scripts:
        case = build_case(record, collection_vars)
        result = probe.run_case(case)
        per_script.append(
            {
                "path": "/".join(record["folder_path"] + [record["item_name"]]),
                "listen": record["listen"],
                "phase": record["phase"],
                "outcome": result.get("outcome", "error"),
                "duration_ms": result.get("duration_ms"),
                "used_api": list(result.get("used_api", [])),
                "diagnostics": list(result.get("diagnostics", [])),
                "transcript": result.get("transcript"),
                "probe_error": result.get("probe_error"),
                "ok": result.get("ok", False),
            }
        )

    stats["scripts_run"] = len(per_script)
    outcomes = Counter(record["outcome"] for record in per_script)
    stats["outcomes"] = {key: outcomes[key] for key in sorted(outcomes)}
    used: dict[str, int] = {}
    for record in per_script:
        for api in record["used_api"]:
            used[api] = used.get(api, 0) + 1
    stats["used_api"] = dict(sorted(used.items(), key=lambda kv: (-kv[1], kv[0])))
    stats["duration_ms_total"] = sum(
        record["duration_ms"] for record in per_script if record["duration_ms"]
    )

    diag_map: dict[tuple[str, str], dict[str, Any]] = {}
    for record in per_script:
        for diag in record["diagnostics"]:
            if not isinstance(diag, dict):
                continue
            code = diag.get("code")
            api = diag.get("api")
            key = (str(code), str(api))
            bucket = diag_map.setdefault(
                key, {"code": str(code), "api": str(api), "count": 0, "message": ""}
            )
            bucket["count"] += 1
            if not bucket["message"] and diag.get("message"):
                bucket["message"] = str(diag["message"])
    stats["diagnostics"] = sorted(
        diag_map.values(), key=lambda d: (d["code"], d["api"])
    )

    transcript_stats = stats["transcript"]
    for record in per_script:
        transcript = record["transcript"]
        if not isinstance(transcript, dict):
            if record["probe_error"]:
                transcript_stats["errors"] += 1
            continue
        tests = transcript.get("tests")
        if isinstance(tests, list):
            transcript_stats["tests"] += len(tests)
            transcript_stats["tests_passed"] += sum(
                1 for test in tests if isinstance(test, dict) and test.get("passed")
            )
            transcript_stats["tests_failed"] += sum(
                1 for test in tests if isinstance(test, dict) and not test.get("passed")
            )
        for field, key in (
            ("logs", "logs"),
            ("errors", "errors"),
            ("child_requests", "child_requests"),
            ("scope_writes", "scope_writes"),
        ):
            value = transcript.get(field)
            if isinstance(value, list):
                transcript_stats[key] += len(value)
        if record["probe_error"]:
            transcript_stats["errors"] += 1

    stats["scripts"] = per_script
    return stats


# ---------------------------------------------------------------------------
# Aggregation, gate, reports
# ---------------------------------------------------------------------------


def aggregate(entries: list[dict[str, Any]], collection_stats: list[dict[str, Any]]) -> dict[str, Any]:
    loaded = [stats for stats in collection_stats if stats["loaded"]]
    with_scripts = [stats for stats in loaded if stats["script_count"] > 0]
    script_records = [record for stats in loaded for record in stats.get("scripts", [])]

    used_counter: Counter[str] = Counter()
    for stats in loaded:
        for api, count in stats["used_api"].items():
            used_counter[api] += count
    used_sorted = sorted(used_counter.items(), key=lambda kv: (-kv[1], kv[0]))

    diag_map: dict[tuple[str, str], dict[str, Any]] = {}
    for stats in loaded:
        for diag in stats["diagnostics"]:
            key = (diag["code"], diag["api"])
            bucket = diag_map.setdefault(
                key,
                {"code": diag["code"], "api": diag["api"], "count": 0, "message": ""},
            )
            bucket["count"] += diag.get("count", 1)
            if not bucket["message"]:
                bucket["message"] = diag.get("message", "")
    diagnostics = sorted(diag_map.values(), key=lambda d: (d["code"], d["api"]))

    outcomes_flat = Counter(record["outcome"] for record in script_records)
    transcript = {
        "tests": sum(stats["transcript"]["tests"] for stats in loaded),
        "tests_passed": sum(stats["transcript"]["tests_passed"] for stats in loaded),
        "tests_failed": sum(stats["transcript"]["tests_failed"] for stats in loaded),
        "logs": sum(stats["transcript"]["logs"] for stats in loaded),
        "errors": sum(stats["transcript"]["errors"] for stats in loaded),
        "child_requests": sum(stats["transcript"]["child_requests"] for stats in loaded),
        "scope_writes": sum(stats["transcript"]["scope_writes"] for stats in loaded),
    }

    # api -> list of (collection name, script path, listen) usage sites
    usage_sites: dict[str, list[dict[str, str]]] = defaultdict(list)
    for stats in loaded:
        for record in stats.get("scripts", []):
            for api in record["used_api"]:
                usage_sites[api].append(
                    {
                        "collection": str(stats["name"]),
                        "path": str(record["path"]),
                        "listen": str(record["listen"]),
                    }
                )

    return {
        "entries": entries,
        "collection_stats": collection_stats,
        "loaded": loaded,
        "with_scripts": with_scripts,
        "script_records": script_records,
        "used_counter": used_counter,
        "used_sorted": used_sorted,
        "diagnostics": diagnostics,
        "outcomes_flat": dict(sorted(outcomes_flat.items())),
        "transcript": transcript,
        "usage_sites": usage_sites,
    }


def gate(aggregated: dict[str, Any], supported: set[str]) -> tuple[bool, list[str], list[str]]:
    used = set(aggregated["used_counter"])
    uncovered = sorted(used - supported)
    corpus_nonempty = len(aggregated["entries"]) > 0
    scripts_ran = len(aggregated["script_records"]) > 0
    passed = corpus_nonempty and scripts_ran and not uncovered
    reasons = []
    if not corpus_nonempty:
        reasons.append("corpus is empty (no manifest entries)")
    if not scripts_ran:
        reasons.append("no scripts were run (all collection files missing or unparseable?)")
    if uncovered:
        reasons.append(f"uncovered APIs: {', '.join(uncovered)}")
    return passed, uncovered, reasons


def write_reports(
    out_dir: Path,
    *,
    probe_description: str,
    list_api: dict[str, Any],
    aggregated: dict[str, Any],
    uncovered: list[str],
    passed: bool,
    reasons: list[str],
    started_at: str,
    duration_ms: int,
) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    supported = list_api.get("supported", [])
    modules = list_api.get("modules", [])
    diagnostic_codes = list_api.get("diagnostic_codes", [])

    collections = []
    for stats in aggregated["collection_stats"]:
        row = {
            "index": stats["index"],
            "name": stats["name"],
            "source_url": stats["source_url"],
            "collection_path": stats["collection_path"],
            "loaded": stats["loaded"],
            "load_error": stats["load_error"],
            "script_count": stats["script_count"],
            "scripts_run": stats["scripts_run"],
            "outcomes": stats["outcomes"],
            "used_api": stats["used_api"],
            "diagnostics": stats["diagnostics"],
            "transcript": stats["transcript"],
            "duration_ms_total": stats["duration_ms_total"],
        }
        collections.append(row)

    summary = {
        "corpus_size": len(aggregated["entries"]),
        "collections_loaded": len(aggregated["loaded"]),
        "collections_with_scripts": len(aggregated["with_scripts"]),
        "collection_load_failures": len(aggregated["collection_stats"])
        - len(aggregated["loaded"]),
        "scripts_total": sum(stats["script_count"] for stats in aggregated["loaded"]),
        "scripts_run": len(aggregated["script_records"]),
        "outcomes": aggregated["outcomes_flat"],
        "used_api_distinct": len(aggregated["used_counter"]),
        "used_api_uses": sum(aggregated["used_counter"].values()),
        "supported_total": len(supported),
        "uncovered": uncovered,
        "diagnostics": aggregated["diagnostics"],
        "transcript": aggregated["transcript"],
        "gate_passed": passed,
        "gate_reasons": reasons,
        "duration_ms": duration_ms,
    }

    report = {
        "schema_version": 1,
        "started_at": started_at,
        "probe": {
            "command": probe_description,
            "profile": list_api.get("profile"),
            "supported": supported,
            "modules": modules,
            "diagnostic_codes": diagnostic_codes,
        },
        "summary": summary,
        "used_api": [
            {"api": api, "count": count} for api, count in aggregated["used_sorted"]
        ],
        "uncovered_usages": {
            api: aggregated["usage_sites"].get(api, [])
            for api in uncovered
        },
        "collections": collections,
    }

    json_path = out_dir / "report.json"
    json_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    md_path = out_dir / "report.md"
    md_path.write_text(render_markdown(report, aggregated), encoding="utf-8")


def render_markdown(report: dict[str, Any], aggregated: dict[str, Any]) -> str:
    summary = report["summary"]
    lines: list[str] = []
    lines.append("# MDOK Postman coverage report")
    lines.append("")
    lines.append(f"- Probe: `{report['probe']['command']}`")
    lines.append(f"- Profile: `{report['probe'].get('profile')}`")
    lines.append(f"- Started: {report['started_at']}")
    lines.append(f"- Duration: {summary['duration_ms']} ms")
    lines.append(f"- Gate: **{'PASS' if summary['gate_passed'] else 'FAIL'}**")
    for reason in summary["gate_reasons"]:
        lines.append(f"  - {reason}")
    lines.append("")
    lines.append("## Summary")
    lines.append("")
    lines.append("| Metric | Value |")
    lines.append("| --- | --- |")
    lines.append(f"| Corpus size (manifest entries) | {summary['corpus_size']} |")
    lines.append(f"| Collections loaded | {summary['collections_loaded']} |")
    lines.append(f"| Collections with scripts | {summary['collections_with_scripts']} |")
    lines.append(f"| Collection load failures | {summary['collection_load_failures']} |")
    lines.append(f"| Scripts found | {summary['scripts_total']} |")
    lines.append(f"| Scripts run | {summary['scripts_run']} |")
    lines.append(f"| Used API distinct | {summary['used_api_distinct']} |")
    lines.append(f"| Used API total uses | {summary['used_api_uses']} |")
    lines.append(f"| Supported API (probe) | {summary['supported_total']} |")
    lines.append(f"| Uncovered API | {len(summary['uncovered'])} |")
    lines.append("")
    lines.append("## Outcomes histogram")
    lines.append("")
    if summary["outcomes"]:
        lines.append("| Outcome | Count |")
        lines.append("| --- | --- |")
        for outcome, count in summary["outcomes"].items():
            lines.append(f"| {outcome} | {count} |")
    else:
        lines.append("_no scripts ran_")
    lines.append("")
    lines.append("## Top used APIs")
    lines.append("")
    top = report["used_api"][:20]
    if top:
        lines.append("| API | Uses |")
        lines.append("| --- | --- |")
        for item in top:
            lines.append(f"| `{item['api']}` | {item['count']} |")
    else:
        lines.append("_no APIs used_")
    lines.append("")
    lines.append("## Full used API list")
    lines.append("")
    if report["used_api"]:
        for item in report["used_api"]:
            lines.append(f"- `{item['api']}` × {item['count']}")
    else:
        lines.append("_empty_")
    lines.append("")
    lines.append("## Uncovered API list")
    lines.append("")
    if summary["uncovered"]:
        for api in summary["uncovered"]:
            lines.append(f"- `{api}`")
            for site in report.get("uncovered_usages", {}).get(api, []):
                lines.append(
                    f"  - {site['collection']} :: {site['path']} ({site['listen']})"
                )
    else:
        lines.append("_empty — every API used by the corpus is supported_")
    lines.append("")
    lines.append("## Diagnostics summary")
    lines.append("")
    if summary["diagnostics"]:
        lines.append("| Code | API | Count |")
        lines.append("| --- | --- | --- |")
        for diag in summary["diagnostics"]:
            lines.append(f"| {diag['code']} | `{diag['api']}` | {diag['count']} |")
        lines.append("")
        for diag in summary["diagnostics"]:
            if diag.get("message"):
                lines.append(f"- {diag['code']} `{diag['api']}`: {diag['message']}")
    else:
        lines.append("_no diagnostics_")
    lines.append("")
    lines.append("## Transcript totals")
    lines.append("")
    lines.append("| Metric | Count |")
    lines.append("| --- | --- |")
    for key in (
        "tests",
        "tests_passed",
        "tests_failed",
        "logs",
        "errors",
        "child_requests",
        "scope_writes",
    ):
        lines.append(f"| {key} | {summary['transcript'][key]} |")
    lines.append("")
    lines.append("## Per-collection")
    lines.append("")
    lines.append("| Index | Collection | Scripts | Run | Outcomes | Used API | Load |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- |")
    for stats in aggregated["collection_stats"]:
        index = stats["index"]
        name = stats["name"]
        scripts = stats["script_count"]
        run = stats["scripts_run"]
        outcomes = ", ".join(f"{k}:{v}" for k, v in stats["outcomes"].items()) or "-"
        used = len(stats["used_api"])
        load = "ok" if stats["loaded"] else f"ERR: {stats['load_error']}"
        lines.append(f"| {index} | {name} | {scripts} | {run} | {outcomes} | {used} | {load} |")
    lines.append("")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run the Postman JS corpus through mdok-pm-probe and enforce the "
            "coverage gate (uncovered = used - supported must be empty)."
        )
    )
    parser.add_argument(
        "--probe",
        default=None,
        help=(
            "path to mdok-pm-probe binary (default: target/release/mdok-pm-probe, "
            "fallback: `cargo run -p mdok-quickjs --bin mdok-pm-probe --`)"
        ),
    )
    parser.add_argument(
        "--corpus",
        default="tests/corpus/postman-js",
        help="corpus directory holding corpus.json + collections/ (default: tests/corpus/postman-js)",
    )
    parser.add_argument(
        "--out",
        default="target/postman-coverage",
        help="output directory for report.json + report.md (default: target/postman-coverage)",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="cap the number of collections processed (default: all)",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=MAX_WORKERS,
        help=f"parallel collection workers, capped at {MAX_WORKERS} (default: {MAX_WORKERS})",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    started_at = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    start_wall = time.monotonic()

    repo_root = Path(__file__).resolve().parent.parent
    corpus_dir = Path(args.corpus).resolve()
    out_dir = Path(args.out).resolve()

    try:
        entries = load_manifest(corpus_dir)
    except CorpusError as exc:
        print(f"FATAL {exc}", file=sys.stderr)
        return 1

    if args.limit is not None and args.limit >= 0:
        entries = entries[: args.limit]

    if not entries:
        print("GATE FAIL: corpus is empty (no manifest entries)", file=sys.stderr)
        try:
            write_reports(
                out_dir,
                probe_description="<none>",
                list_api={"supported": [], "modules": [], "diagnostic_codes": []},
                aggregated=aggregate([], []),
                uncovered=[],
                passed=False,
                reasons=["corpus is empty (no manifest entries)"],
                started_at=started_at,
                duration_ms=int((time.monotonic() - start_wall) * 1000),
            )
        except OSError as exc:
            print(f"WARN could not write report: {exc}", file=sys.stderr)
        return 1

    try:
        probe = resolve_probe(args.probe, repo_root)
        list_api = probe.list_api()
    except ProbeUnavailable as exc:
        print(f"FATAL {exc}", file=sys.stderr)
        print(
            "HINT build the probe crate first (cargo build --release -p mdok-quickjs)"
            " or pass --probe PATH",
            file=sys.stderr,
        )
        return 1

    supported = set(list_api.get("supported", []))
    workers = max(1, min(args.workers, MAX_WORKERS))
    print(
        f"corpus: {len(entries)} collections, probe={probe.description}, "
        f"supported={len(supported)}, workers={workers}",
        file=sys.stderr,
    )

    collection_stats: list[dict[str, Any]] = [None] * len(entries)  # type: ignore[list-item]
    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {
            pool.submit(process_collection, entry, corpus_dir, probe): position
            for position, entry in enumerate(entries)
        }
        for future in as_completed(futures):
            position = futures[future]
            try:
                stats = future.result()
            except ProbeUnavailable as exc:
                pool.shutdown(wait=False, cancel_futures=True)
                print(f"FATAL {exc}", file=sys.stderr)
                return 1
            collection_stats[position] = stats
            print(
                f"collection {stats.get('index')} {stats.get('name')}: "
                f"loaded={stats['loaded']} scripts={stats['script_count']}",
                file=sys.stderr,
            )
    # Results are written back by manifest position, so collection_stats is
    # already in deterministic manifest order regardless of completion order.

    aggregated = aggregate(entries, collection_stats)
    passed, uncovered, reasons = gate(aggregated, supported)
    duration_ms = int((time.monotonic() - start_wall) * 1000)

    write_reports(
        out_dir,
        probe_description=probe.description,
        list_api=list_api,
        aggregated=aggregated,
        uncovered=uncovered,
        passed=passed,
        reasons=reasons,
        started_at=started_at,
        duration_ms=duration_ms,
    )

    print(f"reports written to {out_dir / 'report.json'} and {out_dir / 'report.md'}", file=sys.stderr)
    if passed:
        print(
            f"GATE PASS: {len(aggregated['script_records'])} scripts, "
            f"{len(aggregated['used_counter'])} APIs used, uncovered=0",
            file=sys.stderr,
        )
        return 0
    print("GATE FAIL:", file=sys.stderr)
    for reason in reasons:
        print(f"  - {reason}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
