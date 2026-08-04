#!/usr/bin/env python3
"""Run the checked-in MDOK corpus against a built CLI.

The fixture server is intentionally supplied by the caller so plan-only cases
never need network access. Execute-stage cases can use --base-url and the
standard fixture variables documented by the PRD.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


READINESS_ENV_VARS = (
    "MDOK_FIXTURE_READINESS",
    "MDOK_FIXTURE_READY",
    "MDOK_FIXTURE_READINESS_FILE",
    "MDOK_FIXTURE_READY_FILE",
)


def _readiness_payload(value: str | None) -> dict[str, str]:
    if not value:
        return {}
    source = value
    candidate = Path(value).expanduser()
    if candidate.is_file():
        source = candidate.read_text(encoding="utf-8")

    records: list[Any] = []
    try:
        records.append(json.loads(source))
    except json.JSONDecodeError:
        for line in source.splitlines():
            if not line.strip():
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                continue

    candidates: list[dict[str, str]] = []
    for record in records:
        if not isinstance(record, dict):
            continue
        if isinstance(record.get("ready"), dict):
            record = record["ready"]
        aliases = {
            "base_url": ("base_url", "http_base_url"),
            "https_base_url": ("https_base_url",),
            "proxy_url": ("proxy_url",),
            "ca_file": ("ca_file",),
        }
        result = {
            target: str(record[key])
            for target, keys in aliases.items()
            for key in keys
            if key in record and record[key] is not None
        }
        if result:
            candidates.append(result)
    return max(candidates, key=len, default={})


def _load_readiness(args: argparse.Namespace) -> dict[str, str]:
    value = args.fixture_readiness
    if value is None:
        value = next((os.environ[name] for name in READINESS_ENV_VARS if os.environ.get(name)), None)
    return _readiness_payload(value)


def _validate_manifest(root: Path) -> bool:
    validator = root / "mdok-prd/scripts/validate_corpus.py"
    completed = subprocess.run(
        [sys.executable, str(validator)], cwd=root, text=True, capture_output=True
    )
    output = f"{completed.stdout}{completed.stderr}"
    if completed.returncode:
        print("FAIL corpus manifest validation", file=sys.stderr)
        print(output[-4000:], file=sys.stderr)
        return False
    print(output.rstrip())
    return True


def _diagnostic_codes(report: dict[str, Any]) -> list[str]:
    codes: list[str] = []

    def visit(value: Any) -> None:
        if isinstance(value, dict):
            if isinstance(value.get("code"), str) and re.fullmatch(r"MDOK-E\d{3}", value["code"]):
                codes.append(value["code"])
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(report.get("diagnostics", []))
    visit(report.get("documents", []))
    return codes


def _validate_json_report(report: Any, case: dict[str, Any]) -> list[dict[str, Any]]:
    if not isinstance(report, dict):
        raise ValueError("JSON report is not an object")
    required = ("schema_version", "mdok_version", "curl_version", "started_at", "duration_ms", "summary", "documents", "events")
    missing = [key for key in required if key not in report]
    if missing:
        raise ValueError(f"JSON report is missing {', '.join(missing)}")
    if report["schema_version"] != "1":
        raise ValueError("JSON report has an unsupported schema_version")
    if not all(isinstance(report[key], str) for key in ("mdok_version", "curl_version", "started_at")):
        raise ValueError("JSON report version/timestamp fields have the wrong type")
    if not isinstance(report["duration_ms"], (int, float)) or report["duration_ms"] < 0:
        raise ValueError("JSON report duration_ms is invalid")
    if not isinstance(report["summary"], dict) or not isinstance(report["documents"], list):
        raise ValueError("JSON report summary/documents have the wrong shape")
    documents = report["documents"]
    expected_status = "passed" if case["expected"] == "pass" else None
    for document in documents:
        if not isinstance(document, dict) or not isinstance(document.get("path"), str):
            raise ValueError("JSON report document has the wrong shape")
        if document.get("status") not in {"passed", "failed", "error", "skipped", "planned"}:
            raise ValueError("JSON report document has an invalid status")
        if not isinstance(document.get("steps", []), list):
            raise ValueError("JSON report document steps are not an array")
        for step in document.get("steps", []):
            if not isinstance(step, dict) or not isinstance(step.get("name"), str):
                raise ValueError("JSON report step has the wrong shape")
            if not isinstance(step.get("checks", []), list):
                raise ValueError("JSON report step checks are not an array")
    if len(documents) != 1:
        raise ValueError(f"expected one report document, found {len(documents)}")
    if expected_status and documents[0]["status"] != expected_status:
        raise ValueError(f"expected document status {expected_status}, found {documents[0]['status']}")
    if not expected_status and documents[0]["status"] not in {"failed", "error"}:
        raise ValueError(f"expected a failed report document, found {documents[0]['status']}")

    summary = report["summary"]
    if summary.get("documents") != len(documents):
        raise ValueError("JSON report summary.documents does not match documents")
    if not isinstance(report["events"], list):
        raise ValueError("JSON report events are not an array")
    events = report["events"]
    if [event.get("sequence") for event in events] != list(range(len(events))):
        raise ValueError("JSON report event sequences are not contiguous")
    expected_events: list[tuple[str, str | None, str | None]] = []
    for document in documents:
        for step in document.get("steps", []):
            expected_events.append(("step.finished", document["path"], step["name"]))
        expected_events.append(("document.finished", document["path"], None))
    actual_events = []
    for event in events:
        if not isinstance(event, dict) or not isinstance(event.get("kind"), str):
            raise ValueError("JSON report event has the wrong shape")
        actual_events.append((event["kind"], event.get("document"), event.get("step")))
    if actual_events != expected_events:
        raise ValueError(f"JSON report event order mismatch: {actual_events!r} != {expected_events!r}")
    for event, expected in zip(events, expected_events):
        if event.get("status") != documents[0]["status"]:
            raise ValueError("JSON report event status does not match its document")

    expected_error = case.get("error_code")
    codes = _diagnostic_codes(report)
    if expected_error and expected_error not in codes:
        raise ValueError(f"expected error code {expected_error}, found {codes or 'none'}")
    if not expected_error and codes:
        raise ValueError(f"unexpected error code(s): {', '.join(codes)}")
    return events


def _validate_json_lines(output: str, expected_events: list[dict[str, Any]]) -> None:
    lines = [line for line in output.splitlines() if line.strip()]
    try:
        events = [json.loads(line) for line in lines]
    except json.JSONDecodeError as exc:
        raise ValueError(f"JSONL contains invalid JSON: {exc}") from exc
    base_fields = ("sequence", "kind", "document", "step", "status", "message")
    normalized = [
        {field: event.get(field) for field in base_fields}
        for event in events
        if isinstance(event, dict)
    ]
    expected = [
        {field: event.get(field) for field in base_fields}
        for event in expected_events
    ]
    if normalized != expected:
        raise ValueError("JSONL events do not match the JSON report events")
    for event in events:
        if not isinstance(event.get("sequence"), int):
            raise ValueError("JSONL event sequence is not an integer")


def _validate_junit(payload: str, expected_path: str, expected_pass: bool) -> None:
    try:
        root = ET.fromstring(payload)
    except ET.ParseError as exc:
        raise ValueError(f"JUnit output is not XML: {exc}") from exc
    if root.tag != "testsuites":
        raise ValueError("JUnit root element is not <testsuites>")
    suites = list(root.findall("testsuite"))
    if len(suites) != 1 or suites[0].get("name") != expected_path:
        raise ValueError("JUnit testsuite does not identify the report document")
    suite = suites[0]
    cases = list(suite.findall("testcase"))
    failures = [testcase for testcase in cases if testcase.find("failure") is not None]
    try:
        tests_attr = int(suite.get("tests", "-1"))
        failures_attr = int(suite.get("failures", "-1"))
    except ValueError as exc:
        raise ValueError("JUnit tests/failures attributes are not integers") from exc
    if tests_attr != len(cases) or failures_attr != len(failures) or not cases:
        raise ValueError("JUnit counts do not match emitted testcases")
    if expected_pass and failures:
        raise ValueError("passing report emitted JUnit failures")
    if not expected_pass and not failures:
        raise ValueError("error report emitted no JUnit failure")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/mdok")
    parser.add_argument("--index", type=Path, default=Path("tests/corpus/index.jsonl"))
    parser.add_argument("--stage", choices=("plan", "execute", "report", "all"), default="all")
    parser.add_argument(
        "--base-url",
        default=None,
    )
    parser.add_argument(
        "--https-base-url",
        default=None,
    )
    parser.add_argument("--proxy-url", default=None)
    parser.add_argument("--ca-file", default=None)
    parser.add_argument(
        "--fixture-readiness",
        "--fixture-ready",
        default=None,
        help="JSON or JSONL readiness record, or a file containing one",
    )
    parser.add_argument("--fixture-dir", type=Path, default=Path("tests/fixtures"))
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    if not _validate_manifest(root):
        return 1
    readiness = _load_readiness(args)
    base_url = args.base_url or readiness.get("base_url") or os.environ.get(
        "MDOK_FIXTURE_BASE_URL", "http://127.0.0.1:9800"
    )
    https_base_url = args.https_base_url or readiness.get("https_base_url") or os.environ.get(
        "MDOK_FIXTURE_HTTPS_BASE_URL"
    )
    proxy_url = args.proxy_url or readiness.get("proxy_url") or os.environ.get("MDOK_FIXTURE_PROXY_URL")
    ca_file = args.ca_file or readiness.get("ca_file") or os.environ.get("MDOK_FIXTURE_CA_FILE")
    artifact_dir = (root / "tests/artifacts").resolve()
    artifact_dir.mkdir(parents=True, exist_ok=True)
    wrong_ca_file = (root / args.fixture_dir / "files/hello.txt").resolve()
    index = (root / args.index).resolve()
    cases = [json.loads(line) for line in index.read_text(encoding="utf-8").splitlines() if line.strip()]
    if args.stage != "all":
        cases = [case for case in cases if case["stage"] == args.stage]
    if args.limit:
        cases = cases[: args.limit]

    failures = 0
    for case in cases:
        path = root / case["path"]
        mode = "lint" if case["stage"] == "plan" else "test"

        def command_for(output: str, junit_path: Path | None = None) -> list[str]:
            command = [str(root / args.binary), mode, str(path)]
            if output == "json":
                command.append("--json")
            elif output == "json-lines":
                command.append("--json-lines")
            elif junit_path is not None:
                command.extend(["--junit", str(junit_path)])
            if case["stage"] == "plan":
                command.append("--offline")
            if base_url:
                command.extend(["--var", f"base_url={base_url}"])
            # Plan-only fixtures must still resolve the HTTPS template before
            # the policy diagnostic is produced, but they never connect. Use a
            # deterministic placeholder when no TLS listener was supplied.
            case_https_base_url = https_base_url
            if case_https_base_url is None and case["stage"] == "plan":
                case_https_base_url = "https://127.0.0.1:9801"
            if case_https_base_url:
                command.extend(["--var", f"https_base_url={case_https_base_url}"])
            for key, value in {
                "proxy_url": proxy_url,
                "ca_file": ca_file,
                "fixture_text_file": (root / args.fixture_dir / "files/hello.txt").resolve(),
                "fixture_binary_file": (root / args.fixture_dir / "files/binary.bin").resolve(),
                "artifact_dir": artifact_dir,
                "wrong_ca_file": wrong_ca_file,
            }.items():
                if value:
                    command.extend(["--var", f"{key}={value}"])
            if mode == "test" and case.get("error_code") == "MDOK-E700":
                command.extend(["--max-body", "1048575"])
            return command

        if case["stage"] == "report":
            try:
                with tempfile.TemporaryDirectory(prefix="mdok-corpus-") as temp_dir:
                    json_completed = subprocess.run(
                        command_for("json"), cwd=root, text=True, capture_output=True
                    )
                    report = json.loads(json_completed.stdout)
                    events = _validate_json_report(report, case)
                    expected_exit = case["expected"] == "pass"
                    if (json_completed.returncode == 0) != expected_exit:
                        raise ValueError(f"JSON exit status was {json_completed.returncode}")

                    junit_path = Path(temp_dir) / "report.xml"
                    junit_completed = subprocess.run(
                        command_for("junit", junit_path), cwd=root, text=True, capture_output=True
                    )
                    if (junit_completed.returncode == 0) != expected_exit:
                        raise ValueError(f"JUnit exit status was {junit_completed.returncode}")
                    _validate_junit(
                        junit_path.read_text(encoding="utf-8"),
                        str(path),
                        expected_exit,
                    )

                    jsonl_completed = subprocess.run(
                        command_for("json-lines"), cwd=root, text=True, capture_output=True
                    )
                    if (jsonl_completed.returncode == 0) != expected_exit:
                        raise ValueError(f"JSONL exit status was {jsonl_completed.returncode}")
                    _validate_json_lines(jsonl_completed.stdout, events)
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                failures += 1
                print(f"FAIL {case['id']} report acceptance: {exc}", file=sys.stderr)
                continue
            print(f"PASS {case['id']} {case['path']} (JSON/JUnit/JSONL)")
            continue

        completed = subprocess.run(command_for("json"), cwd=root, text=True, capture_output=True)
        output = f"{completed.stdout}\n{completed.stderr}"
        expected_error = case.get("error_code")
        observed_error = None
        match = re.search(r"MDOK-E\d{3}", output)
        if match:
            observed_error = match.group(0)
        passed = completed.returncode == 0 if case["expected"] == "pass" else completed.returncode != 0
        if expected_error:
            passed = passed and observed_error == expected_error
        if not passed:
            failures += 1
            print(
                f"FAIL {case['id']} expected={case['expected']} {expected_error or ''} "
                f"exit={completed.returncode} observed={observed_error or 'none'}",
                file=sys.stderr,
            )
            print(output[-2000:], file=sys.stderr)
        else:
            print(f"PASS {case['id']} {case['path']}")
    print(f"corpus: {len(cases) - failures}/{len(cases)} passed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
