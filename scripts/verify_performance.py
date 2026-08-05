#!/usr/bin/env python3
"""Verify named target and regression budgets in a benchmark artifact."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any


def verify(report: dict[str, Any], strict: bool) -> int:
    budgets = report.get("budgets")
    if not isinstance(budgets, dict):
        print("performance report has no budgets section", file=sys.stderr)
        return 2

    failures = 0
    unavailable = 0
    target_checks = budgets.get("target_budgets", {})
    if not isinstance(target_checks, dict):
        print("performance report has malformed target_budgets", file=sys.stderr)
        return 2
    target_list = list(target_checks.values())
    groups = {
        "targets": target_list,
        "baseline": list(
            budgets.get("baseline", {}).get("comparisons", {}).values()
        )
        if isinstance(budgets.get("baseline"), dict)
        else [],
    }
    for kind, checks in groups.items():
        if not isinstance(checks, list):
            print(f"{kind}: malformed checks", file=sys.stderr)
            return 2
        counts = {"pass": 0, "fail": 0, "unavailable": 0}
        for check in checks:
            status = check.get("status", "unavailable")
            counts[status] = counts.get(status, 0) + 1
            if status == "fail":
                failures += 1
                print(
                    f"FAIL {kind}/{check.get('name')}: "
                    f"actual={check.get('actual', check.get('actual_seconds'))} "
                    f"limit={check.get('limit', check.get('limit_seconds'))} "
                    f"{check.get('unit', 'seconds')}"
                )
            elif status == "unavailable":
                unavailable += 1
                print(f"UNAVAILABLE {kind}/{check.get('name')}")
        print(
            f"{kind}: {counts.get('pass', 0)} passed, "
            f"{counts.get('fail', 0)} failed, "
            f"{counts.get('unavailable', 0)} unavailable"
        )

    if failures:
        return 1
    if strict and unavailable:
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument(
        "--strict",
        action="store_true",
        help="treat unavailable metrics, such as unsupported RSS probes, as failures",
    )
    args = parser.parse_args()
    try:
        report = json.loads(args.report.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"could not read performance report: {error}", file=sys.stderr)
        return 2
    if not isinstance(report, dict):
        print("performance report must contain a JSON object", file=sys.stderr)
        return 2
    return verify(report, args.strict)


if __name__ == "__main__":
    raise SystemExit(main())
