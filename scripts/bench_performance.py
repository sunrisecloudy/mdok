#!/usr/bin/env python3
"""Measure reproducible process-level MDOK performance workloads.

The harness uses only the Python standard library.  It builds the release CLI
once, creates immutable loopback fixtures once, and measures fresh MDOK
processes with a monotonic clock.  The JSON artifact retains raw samples,
machine metadata, target budgets, and optional comparison to a named baseline.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import re
import resource
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target" / "release" / "mdok"
DEFAULT_OUTPUT = ROOT / "target" / "performance-bench.json"
RSS_MAC_RE = re.compile(r"^\s*(\d+)\s+maximum resident set size", re.I | re.M)
RSS_LINUX_RE = re.compile(r"maximum resident set size \(kbytes\):\s*(\d+)", re.I)
MAX_DIAGNOSTIC_BYTES = 2_000

# These are named checklist gates.  They are reported even when no baseline is
# supplied; --fail-on-regression makes a failed or unavailable gate fatal.
TARGET_BUDGETS = {
    "cold_version_p50_ms_under_50": {
        "description": "cold mdok version p50",
        "workload": "version",
        "case": "cold",
        "metric": "wall_seconds.p50",
        "limit": 50.0,
        "unit": "milliseconds",
    },
    "normal_plan_process_p50_ms_under_50": {
        "description": "normal 10 KB/10-step process plan p50",
        "workload": "normal",
        "case": "plan",
        "metric": "wall_seconds.p50",
        "limit": 50.0,
        "unit": "milliseconds",
    },
    "intense_discovery_p50_ms_under_1000": {
        "description": "1,000-document parallel discovery/plan p50",
        "workload": "intense",
        "case": "discovery_1000_jobs_physical_cores",
        "metric": "wall_seconds.p50",
        "limit": 1_000.0,
        "unit": "milliseconds",
    },
    "intense_discovery_rss_p50_mb_under_100": {
        "description": "1,000-document parallel discovery/plan RSS p50",
        "workload": "intense",
        "case": "discovery_1000_jobs_physical_cores",
        "metric": "peak_rss_bytes.p50",
        "limit": 100.0,
        "unit": "mebibytes",
    },
}

WORKLOADS = {
    "normal": {"steps": 10, "prose_bytes": 10 * 1024},
    "intense": {"steps": 100, "prose_bytes": 1 * 1024 * 1024},
}


def timestamp() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="milliseconds").replace(
        "+00:00", "Z"
    )


def bounded_text(value: str) -> str:
    return value[-MAX_DIAGNOSTIC_BYTES:]


def command_text(command: list[str]) -> list[str]:
    return [str(part) for part in command]


def command_output(command: list[str], cwd: Path = ROOT) -> str | None:
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=False,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    value = completed.stdout.strip()
    return value or None


def sysctl_value(name: str) -> str | None:
    if platform.system() != "Darwin":
        return None
    return command_output(["sysctl", "-n", name])


def integer_value(value: str | None) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except ValueError:
        return None


def host_metadata() -> dict[str, Any]:
    physical_cores = integer_value(sysctl_value("hw.physicalcpu"))
    logical_cores = integer_value(sysctl_value("hw.logicalcpu")) or os.cpu_count()
    ram_bytes = integer_value(sysctl_value("hw.memsize"))
    cpu_model = sysctl_value("machdep.cpu.brand_string") or platform.processor() or None
    if sys.platform.startswith("linux"):
        try:
            cpu_lines = Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines()
            cpu_model = next(
                (line.split(":", 1)[1].strip() for line in cpu_lines if line.startswith("model name")),
                cpu_model,
            )
            physical_pairs: set[tuple[str, str]] = set()
            physical_id: str | None = None
            core_id: str | None = None
            for line in cpu_lines + [""]:
                if line.startswith("physical id"):
                    physical_id = line.split(":", 1)[1].strip()
                elif line.startswith("core id"):
                    core_id = line.split(":", 1)[1].strip()
                elif not line.strip():
                    if physical_id is not None and core_id is not None:
                        physical_pairs.add((physical_id, core_id))
                    physical_id = None
                    core_id = None
            physical_cores = len(physical_pairs) or physical_cores
        except OSError:
            pass
        try:
            memory_lines = Path("/proc/meminfo").read_text(encoding="utf-8").splitlines()
            memory = next(
                (line.split()[1] for line in memory_lines if line.startswith("MemTotal:")),
                None,
            )
            ram_bytes = int(memory) * 1024 if memory is not None else ram_bytes
        except (OSError, ValueError):
            pass
    return {
        "os": {
            "system": platform.system(),
            "release": platform.release(),
            "version": platform.version(),
            "platform": platform.platform(),
        },
        "architecture": platform.machine(),
        "cpu": {
            "model": cpu_model,
            "physical_cores": physical_cores,
            "logical_cores": logical_cores,
        },
        "ram_bytes": ram_bytes,
    }


def git_metadata() -> dict[str, Any]:
    commit = command_output(["git", "rev-parse", "HEAD"])
    branch = command_output(["git", "branch", "--show-current"])
    status = command_output(["git", "status", "--porcelain"])
    return {
        "commit": commit,
        "branch": branch,
        "dirty": bool(status),
    }


def toolchain_metadata() -> dict[str, Any]:
    rust_toolchain = ROOT / "rust-toolchain.toml"
    toolchain = None
    if rust_toolchain.exists():
        text = rust_toolchain.read_text(encoding="utf-8")
        match = re.search(r"^channel\s*=\s*[\"']([^\"']+)", text, re.MULTILINE)
        toolchain = match.group(1) if match else None
    return {
        "rustc": command_output(["rustc", "--version"]),
        "cargo": command_output(["cargo", "--version"]),
        "toolchain": toolchain,
        "profile": "release",
    }


def relevant_environment() -> dict[str, str | None]:
    names = (
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_DIR",
        "RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUST_LOG",
    )
    return {name: os.environ.get(name) for name in names}


def markdown_source(label: str, steps: int, prose_bytes: int, endpoint: str) -> str:
    filler = (
        "The benchmark document contains ordinary API notes, examples, and "
        "response guidance.\n"
    )
    source = [
        "# MDOK benchmark document\n\n",
        "```toml mdok vars\n",
        'request_id = "bench-request"\n\n[metadata]\nowner = "performance"\n',
        "```\n\n",
    ]
    for index in range(steps):
        source.extend(
            [
                f"## API step {index}\n\n",
                f"```curl mdok name=step_{index}\n",
                "curl --request POST "
                "--header 'X-Mdok-Request: {{request_id|header}}' "
                f"--header 'X-Mdok-Step: {index}' "
                f"--data '{{\"step\":{index},\"workload\":\"{label}\"}}' "
                f"{endpoint}/api/{index}\n",
                "```\n\n",
                f"```jmespath mdok check=step_{index}\n",
                "status == `200`\n",
                "```\n\n",
                f"```jmespath mdok capture=step_{index}\n",
                f"{{response_id_{index}: body.id}}\n",
                "```\n\n",
            ]
        )
    document = "".join(source)
    while len(document) < prose_bytes:
        document += filler
    return document


def discovery_source(index: int, endpoint: str, target_bytes: int = 2 * 1024) -> str:
    source = (
        f"# Discovery document {index}\n\n"
        f"```curl mdok name=step_{index}\n"
        f"curl {endpoint}/discovery/{index}\n"
        "```\n\n"
    )
    filler = "Discovery workload prose keeps each input near the PRD's 2 KiB target.\n"
    while len(source) < target_bytes:
        source += filler
    return source


class FixtureHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    response_body = b'{"id":"bench-response","ok":true}'

    def do_GET(self) -> None:  # noqa: N802 - stdlib handler API
        self._respond()

    def do_POST(self) -> None:  # noqa: N802 - stdlib handler API
        length = int(self.headers.get("Content-Length", "0"))
        if length:
            self.rfile.read(length)
        self._respond()

    def _respond(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(self.response_body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(self.response_body)

    def log_message(self, _format: str, *_args: Any) -> None:
        return


class FixtureServer:
    def __enter__(self) -> str:
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        return f"http://127.0.0.1:{self.server.server_port}"

    def __exit__(self, *_args: Any) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)


def parse_rss(stderr: str, darwin: bool) -> int | None:
    match = (RSS_MAC_RE if darwin else RSS_LINUX_RE).search(stderr)
    if match is None:
        return None
    # macOS reports bytes; Linux reports KiB.
    return int(match.group(1)) if darwin else int(match.group(1)) * 1024


def child_rss_bytes() -> int:
    value = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    # macOS reports bytes; Linux reports KiB.
    return int(value if sys.platform == "darwin" else value * 1024)


def measure(command: list[str]) -> dict[str, Any]:
    time_binary = "/usr/bin/time" if Path("/usr/bin/time").exists() else None
    darwin = platform.system() == "Darwin"
    wrapped = command
    rss_source = "resource.getrusage"
    if time_binary:
        wrapped = [time_binary, "-l" if darwin else "-v", *command]
        rss_source = "/usr/bin/time"
    rss_before = child_rss_bytes()
    started_at = timestamp()
    started = time.perf_counter()
    completed = subprocess.run(
        wrapped,
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    elapsed = time.perf_counter() - started
    ended_at = timestamp()
    rss = parse_rss(completed.stderr, darwin) if time_binary else None
    if rss is None:
        rss = max(0, child_rss_bytes() - rss_before)
    return {
        "command": command_text(command),
        "started_at": started_at,
        "ended_at": ended_at,
        "seconds": elapsed,
        "rss_bytes": rss,
        "rss_source": rss_source,
        "returncode": completed.returncode,
        "failed": completed.returncode != 0,
        "stderr": bounded_text(completed.stderr),
    }


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


def statistics(values: list[float]) -> dict[str, float] | None:
    if not values:
        return None
    return {
        "min": min(values),
        "p50": percentile(values, 0.50),
        "p95": percentile(values, 0.95),
        "p99": percentile(values, 0.99),
        "max": max(values),
    }


def summarize(
    samples: list[dict[str, Any]],
    warmup_samples: list[dict[str, Any]],
    work_units: int,
    unit: str,
) -> dict[str, Any]:
    successful = [sample for sample in samples if not sample["failed"]]
    warmup_failures = [sample for sample in warmup_samples if sample["failed"]]
    wall = [sample["seconds"] for sample in successful]
    rss = [float(sample["rss_bytes"]) for sample in successful]
    throughput = [work_units / seconds for seconds in wall if seconds > 0]
    failures = [
        {
            "iteration": index + 1,
            "returncode": sample["returncode"],
            "stderr": sample["stderr"],
        }
        for index, sample in enumerate(samples)
        if sample["failed"]
    ]
    return {
        "unit": unit,
        "work_units": work_units,
        "sample_counts": {
            "warmups": len(warmup_samples),
            "measured": len(samples),
            "successful": len(successful),
            "failed": len(failures),
            "warmup_failed": len(warmup_failures),
            "total_processes": len(warmup_samples) + len(samples),
        },
        "failures": failures,
        "warmup_failures": [
            {"returncode": sample["returncode"], "stderr": sample["stderr"]}
            for sample in warmup_failures
        ],
        "wall_seconds": statistics(wall),
        "peak_rss_bytes": statistics(rss),
        "throughput": {
            "unit": f"{unit}/second",
            "per_second": statistics(throughput),
        },
        "raw_samples": samples,
        "raw_warmup_samples": warmup_samples,
    }


def run_command_case(
    command: list[str],
    runs: int,
    warmups: int,
    work_units: int,
    unit: str,
    label: str,
) -> dict[str, Any]:
    warmup_samples = []
    for _ in range(warmups):
        warmup_samples.append(measure(command))
    samples = []
    for iteration in range(runs):
        sample = measure(command)
        sample["iteration"] = iteration + 1
        samples.append(sample)
    result = summarize(samples, warmup_samples, work_units, unit)
    result["name"] = label
    return result


def run_case(
    binary: Path,
    config: Path,
    document: Path,
    mode: str,
    runs: int,
    warmups: int,
    options: list[str] | None = None,
    work_units: int = 1,
    unit: str = "invocation",
    label: str | None = None,
) -> dict[str, Any]:
    command = [
        str(binary),
        "--config",
        str(config),
        *(options or []),
        "--json",
        mode,
        str(document),
    ]
    return run_command_case(
        command,
        runs,
        warmups,
        work_units,
        unit,
        label or mode,
    )


def run_version(binary: Path, runs: int) -> dict[str, Any]:
    # No warmup is intentional: every measured sample is a fresh cold process.
    return run_command_case(
        [str(binary), "--json", "version"],
        runs,
        0,
        1,
        "invocation",
        "cold",
    )


def build_binary(binary: Path, skip_build: bool) -> dict[str, Any]:
    started_at = timestamp()
    started = time.perf_counter()
    if not skip_build:
        subprocess.run(
            ["cargo", "build", "--locked", "--release", "-p", "mdok-cli"],
            cwd=ROOT,
            check=True,
        )
    if not binary.exists():
        raise RuntimeError(f"release binary was not produced: {binary}")
    return {
        "command": ["cargo", "build", "--locked", "--release", "-p", "mdok-cli"],
        "started_at": started_at,
        "ended_at": timestamp(),
        "seconds": time.perf_counter() - started,
        "profile": "release",
        "skipped": skip_build,
        "included_in_timed_samples": False,
    }


def binary_metadata(binary: Path) -> dict[str, Any]:
    digest = hashlib.sha256()
    with binary.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return {
        "path": str(binary),
        "sha256": digest.hexdigest(),
        "bytes": binary.stat().st_size,
    }


def physical_core_count(metadata: dict[str, Any]) -> int:
    cpu = metadata.get("cpu", {})
    return max(1, int(cpu.get("physical_cores") or cpu.get("logical_cores") or 1))


def job_variants(metadata: dict[str, Any]) -> list[tuple[str, int]]:
    physical = physical_core_count(metadata)
    values = [
        ("jobs_1", 1),
        ("jobs_2", 2),
        ("jobs_physical_cores", physical),
        ("jobs_overcommitted", max(physical + 1, physical * 2)),
    ]
    seen: set[int] = set()
    return [(name, jobs) for name, jobs in values if not (jobs in seen or seen.add(jobs))]


def nested_metric(case: dict[str, Any], path: str) -> float | None:
    value: Any = case
    for part in path.split("."):
        if not isinstance(value, dict):
            return None
        value = value.get(part)
    return float(value) if isinstance(value, (int, float)) else None


def case_from_results(results: dict[str, Any], workload: str, case: str) -> dict[str, Any] | None:
    return results.get("workloads", {}).get(workload, {}).get("cases", {}).get(case)


def budget_status(actual: float | None, limit: float) -> str:
    if actual is None:
        return "unavailable"
    return "pass" if actual <= limit else "fail"


def collect_budgets(
    results: dict[str, Any],
    baseline: dict[str, Any] | None,
    baseline_name: str,
    baseline_path: Path | None,
    baseline_commit: str | None,
    regression_percent: float,
) -> dict[str, Any]:
    targets: dict[str, Any] = {}
    for name, definition in TARGET_BUDGETS.items():
        case = case_from_results(results, definition["workload"], definition["case"])
        actual = nested_metric(case or {}, definition["metric"])
        if definition["unit"] == "milliseconds" and actual is not None:
            actual *= 1_000
        if definition["unit"] == "mebibytes" and actual is not None:
            actual /= 1024 * 1024
        status = budget_status(actual, definition["limit"])
        targets[name] = {
            "description": definition["description"],
            "metric": f"{definition['workload']}.{definition['case']}.{definition['metric']}",
            "actual": actual,
            "limit": definition["limit"],
            "unit": definition["unit"],
            "status": status,
        }

    comparisons: dict[str, Any] = {}
    for workload, workload_result in results.get("workloads", {}).items():
        for case_name, case in workload_result.get("cases", {}).items():
            actual = nested_metric(case, "wall_seconds.p50")
            baseline_case = case_from_results(baseline or {}, workload, case_name)
            previous = nested_metric(baseline_case or {}, "wall_seconds.p50")
            limit = previous * (1 + regression_percent / 100) if previous is not None else None
            name = f"{workload}_{case_name}_p50_within_{regression_percent:g}pct_of_{baseline_name}"
            status = "not_compared"
            if actual is not None and limit is not None:
                status = "pass" if actual <= limit else "fail"
            elif actual is None:
                status = "unavailable"
            comparisons[name] = {
                "metric": f"{workload}.{case_name}.wall_seconds.p50",
                "actual_seconds": actual,
                "baseline_seconds": previous,
                "limit_seconds": limit,
                "allowed_regression_percent": regression_percent,
                "status": status,
            }

    target_values = [item["status"] for item in targets.values()]
    target_overall = "fail" if "fail" in target_values else (
        "incomplete" if "unavailable" in target_values else "pass"
    )
    comparison_values = [item["status"] for item in comparisons.values()]
    baseline_overall = "not_compared"
    if baseline is not None:
        baseline_overall = "fail" if "fail" in comparison_values else (
            "incomplete" if "unavailable" in comparison_values else "pass"
        )
    overall = "fail" if "fail" in (target_overall, baseline_overall) else (
        "incomplete" if "incomplete" in (target_overall, baseline_overall) else "pass"
    )
    return {
        "overall_status": overall,
        "target_budgets": targets,
        "baseline": {
            "name": baseline_name,
            "commit": (
                (baseline or {}).get("metadata", {}).get("git", {}).get("commit")
                if baseline is not None
                else baseline_commit
            ),
            "path": str(baseline_path) if baseline_path else None,
            "status": baseline_overall,
            "regression_percent": regression_percent,
            "comparisons": comparisons,
        },
    }


def load_baseline(path: Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError(f"cannot read baseline {path}: {error}") from error
    if not isinstance(value, dict):
        raise RuntimeError(f"baseline must contain a JSON object: {path}")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--baseline-name", default="mdok-performance-checklist-v1")
    parser.add_argument("--baseline-commit")
    parser.add_argument("--regression-percent", type=float, default=10.0)
    parser.add_argument("--fail-on-regression", action="store_true")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if args.runs < 1 or args.warmups < 0:
        parser.error("--runs must be positive and --warmups cannot be negative")
    if args.regression_percent < 0:
        parser.error("--regression-percent cannot be negative")

    started_at = timestamp()
    baseline = load_baseline(args.baseline)
    binary = args.binary.resolve()
    metadata = host_metadata()
    results: dict[str, Any] = {
        "schema_version": "mdok-performance-v2",
        "started_at": started_at,
        "metadata": {
            **metadata,
            "git": git_metadata(),
            "python": platform.python_version(),
            "rust_cargo": toolchain_metadata(),
            "environment": relevant_environment(),
        },
        "binary": None,
        "build": None,
        "measurement": {
            "runs": args.runs,
            "warmups": args.warmups,
            "clock": "time.perf_counter",
            "fresh_process_per_sample": True,
            "stdout_captured": False,
            "fixture_setup_included_in_samples": False,
        },
        "cli_contract": {
            "version_json": True,
            "global_jobs": True,
            "directory_discovery": True,
            "modes": ["lint", "plan", "test"],
        },
        "workloads": {},
    }

    build = build_binary(binary, args.skip_build)
    results["build"] = build
    results["binary"] = binary_metadata(binary)
    results["workloads"]["version"] = {"cases": {"cold": run_version(binary, args.runs)}}

    fixture_started_at = timestamp()
    with tempfile.TemporaryDirectory(prefix="mdok-performance-") as temporary:
        temp_dir = Path(temporary)
        with FixtureServer() as endpoint:
            config = temp_dir / "mdok.toml"
            config.write_text(
                "[policy]\n"
                "allow_private_network = true\n"
                'allowed_hosts = ["127.0.0.1"]\n',
                encoding="utf-8",
            )
            workload_documents: dict[str, Path] = {}
            for label, spec in WORKLOADS.items():
                document = temp_dir / f"{label}.md"
                document.write_text(
                    markdown_source(label, spec["steps"], spec["prose_bytes"], endpoint),
                    encoding="utf-8",
                )
                workload_documents[label] = document

            discovery_dir = temp_dir / "discovery-1000"
            discovery_dir.mkdir()
            discovery_count = 1_000
            for index in range(discovery_count):
                (discovery_dir / f"document-{index:04d}.md").write_text(
                    discovery_source(index, endpoint), encoding="utf-8"
                )
            fixture_setup_ended_at = timestamp()
            discovery_bytes = sum(
                path.stat().st_size for path in discovery_dir.iterdir()
            )

            for label, spec in WORKLOADS.items():
                document = workload_documents[label]
                cases: dict[str, Any] = {}
                for mode in ("lint", "plan", "test"):
                    print(f"{label}/{mode}: measuring {args.runs} runs", flush=True)
                    cases[mode] = run_case(
                        binary,
                        config,
                        document,
                        mode,
                        args.runs,
                        args.warmups,
                        work_units=spec["steps"],
                        unit="steps",
                    )
                if label == "intense":
                    for variant, jobs in job_variants(metadata):
                        name = f"discovery_1000_{variant}"
                        print(f"intense/{name}: measuring {args.runs} runs", flush=True)
                        cases[name] = run_case(
                            binary,
                            config,
                            discovery_dir,
                            "plan",
                            args.runs,
                            args.warmups,
                            options=["--jobs", str(jobs)],
                            work_units=discovery_count,
                            unit="documents",
                            label=name,
                        )
                results["workloads"][label] = {
                    "document_bytes": document.stat().st_size,
                    "steps": spec["steps"],
                    "cases": cases,
                }
            results["workloads"]["intense"]["discovery_documents"] = discovery_count
            results["workloads"]["intense"]["discovery_document_bytes"] = discovery_bytes
            results["workloads"]["intense"]["discovery_jobs"] = {
                name: jobs for name, jobs in job_variants(metadata)
            }
    results["fixture_setup"] = {
        "started_at": fixture_started_at,
        "ended_at": fixture_setup_ended_at,
        "included_in_timed_samples": False,
        "normal_document_bytes": results["workloads"]["normal"]["document_bytes"],
        "intense_document_bytes": results["workloads"]["intense"]["document_bytes"],
        "discovery_documents": discovery_count,
        "discovery_bytes": discovery_bytes,
    }

    results["budgets"] = collect_budgets(
        results,
        baseline,
        args.baseline_name,
        args.baseline,
        args.baseline_commit,
        args.regression_percent,
    )
    results["ended_at"] = timestamp()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")

    for workload, workload_result in results["workloads"].items():
        for mode, case in workload_result.get("cases", {}).items():
            wall = case["wall_seconds"]
            if wall is None:
                print(f"{workload}/{mode}: no successful samples")
                continue
            rss = case["peak_rss_bytes"]["p50"] / (1024 * 1024)
            print(
                f"{workload}/{mode}: p50={wall['p50'] * 1000:.2f} ms "
                f"p95={wall['p95'] * 1000:.2f} ms p99={wall['p99'] * 1000:.2f} ms "
                f"throughput-p50={case['throughput']['per_second']['p50']:.2f}/s "
                f"rss-p50={rss:.2f} MiB failures={case['sample_counts']['failed']}"
            )
    print(f"budget status: {results['budgets']['overall_status']}")
    print(f"wrote {args.output}")
    if args.fail_on_regression and results["budgets"]["overall_status"] != "pass":
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
