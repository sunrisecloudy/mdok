#!/usr/bin/env python3
"""Emit deterministic in-toto/SLSA-style provenance for an MDOK bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def run(root: Path, *command: str) -> str:
    return subprocess.check_output(command, cwd=root, text=True).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve()
    curl_version = (root / "vendor/curl.version").read_text(encoding="utf-8").strip()
    curl_checksum = (root / "vendor/curl.sha256").read_text(encoding="utf-8").split()[0]
    source_revision = run(root, "git", "rev-parse", "HEAD")
    rustc = run(root, "rustc", "-vV")
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    provenance = {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [
            {
                "name": binary.name,
                "digest": {"sha256": digest},
            }
        ],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://mdok.dev/build/package/v1",
                "externalParameters": {
                    "target": args.target,
                    "curl_version": curl_version,
                    "rustc": rustc,
                },
                "resolvedDependencies": [
                    {
                        "uri": "git+workspace",
                        "digest": {"sha1": source_revision},
                    },
                    {
                        "uri": f"https://curl.se/download/curl-{curl_version}.tar.xz",
                        "digest": {"sha256": curl_checksum},
                    },
                ],
            },
            "runDetails": {
                "builder": {"id": "https://mdok.dev/builders/package.sh"},
            },
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote provenance for {binary} to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
