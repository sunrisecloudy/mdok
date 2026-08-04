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
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/mdok")
    parser.add_argument("--index", type=Path, default=Path("tests/corpus/index.jsonl"))
    parser.add_argument("--stage", choices=("plan", "execute", "report", "all"), default="all")
    parser.add_argument(
        "--base-url",
        default=os.environ.get("MDOK_FIXTURE_BASE_URL", "http://127.0.0.1:9800"),
    )
    parser.add_argument("--fixture-dir", type=Path, default=Path("tests/fixtures"))
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
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
        command = [str(root / args.binary), mode, str(path), "--json", "--offline"]
        if mode == "test":
            command.remove("--offline")
            if args.base_url:
                command.extend(["--var", f"base_url={args.base_url}"])
        elif args.base_url:
            command.extend(["--var", f"base_url={args.base_url}"])
        if args.base_url:
            command.extend(["--var", f"https_base_url={args.base_url}"])
        command.extend(
            [
                "--var",
                f"fixture_text_file={(root / args.fixture_dir / 'files/hello.txt').resolve()}",
                "--var",
                f"fixture_binary_file={(root / args.fixture_dir / 'files/binary.bin').resolve()}",
                "--var",
                f"artifact_dir={artifact_dir}",
                "--var",
                f"wrong_ca_file={wrong_ca_file}",
            ]
        )
        if mode == "test" and case.get("error_code") == "MDOK-E700":
            command.extend(["--max-body", "1048575"])
        completed = subprocess.run(command, cwd=root, text=True, capture_output=True)
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
