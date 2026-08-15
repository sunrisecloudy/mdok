#!/usr/bin/env python3
from __future__ import annotations
import json
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
manifest_path = root / "tests/corpus/index.jsonl"
errors: list[str] = []
items = []
for line_no, line in enumerate(manifest_path.read_text(encoding="utf-8").splitlines(), 1):
    try:
        item = json.loads(line)
    except Exception as exc:
        errors.append(f"manifest line {line_no}: {exc}")
        continue
    items.append(item)

ids = set()
paths = set()
for item in items:
    if item["id"] in ids: errors.append(f"duplicate id {item['id']}")
    ids.add(item["id"])
    if item["path"] in paths: errors.append(f"duplicate path {item['path']}")
    paths.add(item["path"])
    path = root / item["path"]
    if not path.is_file():
        errors.append(f"missing file {item['path']}")
        continue
    text = path.read_text(encoding="utf-8")
    if not text.startswith(f"# {item['id']}:"):
        errors.append(f"{item['path']}: header id mismatch")
    marker = f"mdok-corpus id={item['id']}"
    if marker not in text:
        errors.append(f"{item['path']}: missing metadata marker")
    if text.count("```") % 2:
        errors.append(f"{item['path']}: odd number of triple-backtick markers")
    if item["expected"] == "error" and not item.get("error_code"):
        errors.append(f"{item['path']}: error case lacks error_code")
    if item["expected"] == "pass" and item.get("error_code") is not None:
        errors.append(f"{item['path']}: pass case has error_code")

actual = {str(p.relative_to(root)).replace('\\\\','/') for p in (root/'tests/corpus').rglob('*.md')}
listed = {x['path'] for x in items}
for p in sorted(actual - listed): errors.append(f"unlisted corpus file {p}")
for p in sorted(listed - actual): errors.append(f"manifest references absent file {p}")

if len(items) != 497:
    errors.append(f"expected 497 cases, found {len(items)}")

if errors:
    print("Corpus validation failed:", file=sys.stderr)
    for error in errors: print(f"- {error}", file=sys.stderr)
    raise SystemExit(1)

print(f"Corpus OK: {len(items)} Markdown fixtures, {len(ids)} unique IDs, {len(actual)} files")
