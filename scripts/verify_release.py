#!/usr/bin/env python3
"""Verify MDOK release signatures, checksums, and manifest binding."""

from __future__ import annotations

import argparse
from pathlib import Path

from release_signing import ReleaseSigningError, verify_release


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key", type=Path, required=True, help="trusted Ed25519 PEM public or private key")
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    try:
        verify_release(key_path=args.key, manifest_path=args.manifest)
    except ReleaseSigningError as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
