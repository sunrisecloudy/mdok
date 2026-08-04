#!/usr/bin/env python3
"""Verify that every checked-in curl patch is present in the vendored tree."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    patch_dir = root / "vendor/patches/curl"
    patches = sorted(patch_dir.glob("*.patch"))
    if not patches:
        print("No curl patches found", file=sys.stderr)
        return 1

    for patch in patches:
        checked = subprocess.run(
            [
                "git",
                "apply",
                "--check",
                "--reverse",
                "--directory=vendor/curl",
                "-p1",
                str(patch),
            ],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if checked.returncode:
            print(f"curl patch is not applied: {patch}", file=sys.stderr)
            print(checked.stderr.rstrip(), file=sys.stderr)
            return 1
    print(f"Verified {len(patches)} applied curl patch(es)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
