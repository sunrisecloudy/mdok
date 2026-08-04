#!/usr/bin/env python3
"""Sign MDOK release artifacts and write a deterministic release manifest."""

from __future__ import annotations

import argparse
from pathlib import Path

from release_signing import ReleaseSigningError, sign_release


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key", type=Path, required=True, help="Ed25519 PEM private signing key")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--artifact", type=Path, action="append", required=True)
    parser.add_argument("--source-state", type=Path, required=True)
    args = parser.parse_args()
    try:
        sign_release(
            key_path=args.key,
            manifest_path=args.manifest,
            version=args.version,
            target=args.target,
            artifacts=args.artifact,
            source_state_path=args.source_state,
        )
    except ReleaseSigningError as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
