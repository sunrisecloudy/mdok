#!/usr/bin/env python3
"""Emit deterministic in-toto/SLSA-style provenance for an MDOK bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

from release_signing import ReleaseSigningError, git_source_state, read_source_state, sha256_file


def run(root: Path, *command: str) -> str:
    return subprocess.check_output(command, cwd=root, text=True).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-archive", type=Path)
    parser.add_argument("--source-state", type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve()
    curl_version = (root / "vendor/curl.version").read_text(encoding="utf-8").strip()
    curl_checksum = (root / "vendor/curl.sha256").read_text(encoding="utf-8").split()[0]
    source_state = (
        read_source_state(args.source_state)
        if args.source_state is not None
        else git_source_state(root)
    )
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
                    "source_revision": source_state["revision"],
                    "source_archive_revision": source_state["revision"],
                    "working_tree_dirty": source_state["working_tree_dirty"],
                    "working_tree_status_sha256": source_state["working_tree_status_sha256"],
                },
                "resolvedDependencies": [
                    {
                        "uri": "git+workspace",
                        "digest": {"sha1": source_state["revision"]},
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
    if args.source_archive is not None:
        provenance["predicate"]["buildDefinition"]["externalParameters"][
            "source_archive_sha256"
        ] = sha256_file(args.source_archive.resolve())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote provenance for {binary} to {args.output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReleaseSigningError as error:
        raise SystemExit(f"generate_provenance.py: {error}") from error
