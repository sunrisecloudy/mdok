#!/usr/bin/env python3
from __future__ import annotations
import csv
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
source = root / "repo-skeleton/vendor/curl/src/tool_listhelp.c"
policy = root / "specs/curl-option-policy.csv"
if not source.is_file():
    raise SystemExit(f"missing {source}; vendor curl first")

text = source.read_text(encoding="utf-8")
options = set()
for match in re.finditer(r'\{\s*"([^"]*--[a-zA-Z0-9][^"]*)"', text):
    help_head = match.group(1)
    for option in re.findall(r'--[a-zA-Z0-9][a-zA-Z0-9.-]*', help_head):
        options.add(option)

classified = {}
with policy.open(newline="", encoding="utf-8") as fh:
    for row in csv.DictReader(fh):
        classified[row["option"]] = row

missing = sorted(option for option in options if option not in classified)
if missing:
    print("Unclassified curl options:", file=sys.stderr)
    for option in missing: print(option, file=sys.stderr)
    raise SystemExit(1)
print(f"All {len(options)} curl long options are classified")
