#!/usr/bin/env python3
"""Keep the MDOK curl-option policy aligned with the pinned curl tool list.

The generated default for an option that MDOK does not implement is
``unsupported``. That is deliberately fail-closed: adding a curl release
cannot silently make a newly introduced option executable.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "vendor/curl/src/tool_listhelp.c"
POLICY = ROOT / "specs/curl-option-policy.csv"


def curl_options() -> set[str]:
    text = SOURCE.read_text(encoding="utf-8")
    return {
        option
        for match in re.finditer(r'\{\s*"([^"{]*--[A-Za-z0-9][^"}]*)"', text)
        for option in re.findall(r"--[A-Za-z0-9][A-Za-z0-9.-]*", match.group(1))
    }


def policy_rows() -> dict[str, dict[str, str]]:
    with POLICY.open(newline="", encoding="utf-8") as handle:
        return {row["option"]: row for row in csv.DictReader(handle)}


def missing_options(options: set[str], rows: dict[str, dict[str, str]]) -> list[str]:
    return sorted(options - rows.keys())


def write_policy(options: set[str], rows: dict[str, dict[str, str]]) -> None:
    for option in options:
        rows.setdefault(
            option,
            {
                "option": option,
                "classification": "unsupported",
                "area": "unclassified-curl-option",
            },
        )
    with POLICY.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["option", "classification", "area"],
            lineterminator="\n",
        )
        writer.writeheader()
        for option in sorted(rows):
            writer.writerow(rows[option])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--update",
        action="store_true",
        help="add newly introduced curl options as fail-closed unsupported rows",
    )
    args = parser.parse_args()
    if not SOURCE.is_file():
        print(f"missing vendored curl option source: {SOURCE}", file=sys.stderr)
        return 1
    if not POLICY.is_file():
        print(f"missing option policy: {POLICY}", file=sys.stderr)
        return 1
    options = curl_options()
    rows = policy_rows()
    missing = missing_options(options, rows)
    if args.update:
        write_policy(options, rows)
        missing = missing_options(options, policy_rows())
    if missing:
        print("Unclassified curl options:", file=sys.stderr)
        print("\n".join(missing), file=sys.stderr)
        return 1
    print(f"All {len(options)} vendored curl long options are classified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
