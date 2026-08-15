#!/usr/bin/env python3
"""Golden-diff harness: pin normalized mdok CLI outputs for port parity.

Runs a deterministic, mostly-offline case set against any mdok binary and
compares normalized stdout/exit codes against committed golden files under
`tests/golden/`. This is the acceptance gate for behavior-compatible
re-implementations (for example the planned Go port): byte-level drift in
report shapes, plan output, diagnostics, importer output, record/replay
recordings, and edge-case error paths fails the run.

Volatile values (durations, run ids, timestamps, ports, temp paths, build
version strings, OS error text) are normalized before comparison; everything
else must match exactly.

Usage:
    python3 scripts/run_golden_diff.py --binary target/debug/mdok          # verify
    python3 scripts/run_golden_diff.py --update                            # re-capture

Cases:
    version            mdok version --json
    corpus-lint/plan/list  lint/plan/list --json over every plan-stage corpus doc
    import-postman     mdok import postman on a minimal collection
    e001-invalid-utf8  lint of a binary (non-UTF-8) document
    e800-report-write  lint with an unwritable --report target
    record-replay      record a loopback request, then replay --strict (spawns
                       the fixture test-server; the only network case)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import select
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
GOLDEN = ROOT / "tests" / "golden"
INDEX = ROOT / "tests" / "corpus" / "index.jsonl"
DEFAULT_BINARY = ROOT / "target" / "debug" / "mdok"
DEFAULT_SERVER = ROOT / "target" / "debug" / "test-server"

NORMALIZERS: list[tuple[str, str]] = [
    (r'"duration_ms":\d+', '"duration_ms":N'),
    # Sub-millisecond phase timings are inherently variable floats.
    (r'"([a-z_]+_ms)": \d+\.\d+', r'"\1": $MS'),
    (r'"(local_port|primary_port)": \d+', r'"\1": $PORT'),
    (r'"duration_ms": \d+', '"duration_ms": N'),
    (r'unix-ms:\d+', 'unix-ms:N'),
    (r'"run_id": ?"run-[0-9a-f]+"', '"run_id": "run-$ID"'),
    (r"\d{4}-\d\d-\d\dT[\d:.]+(Z|[+-]\d\d:?\d\d)?", "$TIME"),
    (r"127\.0\.0\.1:\d+", "127.0.0.1:$PORT"),
    (r"/private/var/folders/\S+|/var/folders/\S+|/tmp/\S+", "$TMP"),
    (r'"mdok_version":"[^"]*"', '"mdok_version":"$VER"'),
    (r'"libcurl":"[^"]*"', '"libcurl":"$VER"'),
    (r'"tls":"[^"]*"', '"tls":"$TLS"'),
    (r'"features":\{[^{}]*\}', '"features":{$FEATURES}'),
    # The recording's source hash covers the recorded URL, which contains the
    # fixture server's dynamic port; the recording text itself is pinned.
    (r'"source_sha256": ?"[0-9a-f]{64}"', '"source_sha256": "$SHA"'),
    # Hashes over content that embeds the fixture server's dynamic port.
    (r'"sha256": ?"[0-9a-f]{64}"', '"sha256": "$SHA"'),
    (r'"message":"could not write report [^"]*"', '"message":"could not write report $ERR"'),
]


def normalize(text: str) -> str:
    for pattern, replacement in NORMALIZERS:
        text = re.sub(pattern, replacement, text)
    return text


def run(binary: Path, args: list[str], cwd: Path | None = None, env: dict[str, str] | None = None,
        stdin_bytes: bytes | None = None) -> dict[str, Any]:
    merged = dict(os.environ)
    if env:
        merged.update(env)
    completed = subprocess.run(
        [str(binary), *args],
        cwd=str(cwd or ROOT),
        env=merged,
        input=stdin_bytes,
        capture_output=True,
        timeout=120,
    )
    return {
        "exit": completed.returncode,
        "stdout": normalize(completed.stdout.decode("utf-8", errors="replace")),
        "stderr": normalize(completed.stderr.decode("utf-8", errors="replace")),
    }


def plan_stage_cases() -> list[dict[str, Any]]:
    cases = []
    for line in INDEX.read_text(encoding="utf-8").splitlines():
        item = json.loads(line)
        if item.get("stage") == "plan" and not item.get("requires"):
            cases.append(item)
    return cases


def capture_corpus(binary: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    for op in ("lint", "plan", "list"):
        records = []
        for case in plan_stage_cases():
            result = run(binary, [op, case["path"], "--json"])
            records.append({"id": case["id"], **result})
        out[f"corpus-{op}"] = "\n".join(json.dumps(r, sort_keys=True) for r in records) + "\n"
    return out


def capture_static(binary: Path, directory: Path) -> dict[str, str]:
    out: dict[str, str] = {}

    out["version"] = json.dumps(run(binary, ["version", "--json"]), sort_keys=True) + "\n"

    collection = ROOT / "tests" / "mcp" / "postman-minimal.json"
    target = directory / "imported.md"
    out["import-postman"] = json.dumps(
        run(binary, ["import", "postman", "--out", str(target), "--json", str(collection)]),
        sort_keys=True,
    ) + "\n"
    imported = target.read_text(encoding="utf-8") if target.is_file() else ""
    out["import-postman"] += normalize(imported)

    invalid = directory / "invalid-utf8.md"
    invalid.write_bytes(b"# Broken\n\n```curl mdok name=a\ncurl https://x.test/\xff\xfe\n```\n")
    out["e001-invalid-utf8"] = json.dumps(
        run(binary, ["lint", str(invalid), "--json"]), sort_keys=True
    ) + "\n"

    blocker = directory / "not-a-dir"
    blocker.write_text("occupied", encoding="utf-8")
    out["e800-report-write"] = json.dumps(
        run(binary, ["lint", "--json", "--report", str(blocker / "nested" / "report.json"),
                     "tests/mcp/health.md"]),
        sort_keys=True,
    ) + "\n"
    return out


def wait_ready(process: subprocess.Popen, path: Path, timeout: float = 30.0) -> dict[str, str]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError("fixture test-server exited early")
        ready, _, _ = select.select([process.stdout], [], [], 0.2)
        if ready:
            line = process.stdout.readline()
            if line.strip():
                return json.loads(line)
    raise RuntimeError("fixture test-server readiness timeout")


def capture_record_replay(binary: Path, server: Path, directory: Path) -> dict[str, str]:
    workspace = directory / "rr"
    workspace.mkdir()
    (workspace / "mdok.toml").write_text(
        '[policy]\nallowed_hosts = ["127.0.0.1"]\n', encoding="utf-8"
    )
    process = subprocess.Popen(
        [str(server), "--listen", "127.0.0.1:0", "--json-ready"],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
    )
    try:
        readiness = wait_ready(process, directory)
        base_url = readiness["http_base_url"]
        recording = workspace / "recording.md"
        record = run(
            binary,
            ["record", "--output", str(recording), "--",
             "curl", f"{base_url}/echo?case=golden"],
            cwd=workspace,
        )
        replay = run(binary, ["replay", "--strict", str(recording)], cwd=workspace)
        source = recording.read_text(encoding="utf-8") if recording.is_file() else ""
        return {
            "record-replay": json.dumps(
                {"record": record, "replay": replay,
                 "recording": normalize(source).replace(base_url, "$BASE_URL")},
                sort_keys=True,
            ) + "\n",
        }
    finally:
        process.terminate()
        process.wait(timeout=10)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--server", type=Path, default=DEFAULT_SERVER,
                        help="fixture test-server for the record/replay case")
    parser.add_argument("--update", action="store_true",
                        help="re-capture golden files instead of verifying")
    parser.add_argument("--skip-record-replay", action="store_true",
                        help="skip the loopback record/replay case")
    args = parser.parse_args()
    if not args.binary.is_file():
        raise SystemExit(f"mdok binary not found: {args.binary}")

    with tempfile.TemporaryDirectory(prefix="mdok-golden-") as raw:
        directory = Path(raw)
        captured = {**capture_static(args.binary, directory), **capture_corpus(args.binary)}
        if not args.skip_record_replay:
            if not args.server.is_file():
                raise SystemExit(f"test-server not found: {args.server}")
            captured.update(capture_record_replay(args.binary, args.server, directory))

    if args.update:
        GOLDEN.mkdir(exist_ok=True)
        for name, content in sorted(captured.items()):
            path = GOLDEN / f"{name}.txt"
            path.write_text(content, encoding="utf-8")
            print(f"captured {name} ({len(content)} bytes)")
        print(f"golden: {len(captured)} files updated under {GOLDEN}")
        return 0

    failed = 0
    for name in sorted(captured):
        path = GOLDEN / f"{name}.txt"
        if not path.is_file():
            print(f"FAIL {name}: golden file missing (run with --update)")
            failed += 1
            continue
        expected = path.read_text(encoding="utf-8")
        if expected == captured[name]:
            print(f"PASS {name}")
        else:
            failed += 1
            print(f"FAIL {name}: output drifted from golden {path}")
            for i, (a, b) in enumerate(zip(expected.splitlines(), captured[name].splitlines())):
                if a != b:
                    print(f"  first difference at line {i + 1}:")
                    print(f"    golden: {a[:240]}")
                    print(f"    actual: {b[:240]}")
                    break
            else:
                print(f"  length differs: golden {len(expected)} vs actual {len(captured[name])}")
    print(f"golden-diff: {len(captured) - failed}/{len(captured)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
