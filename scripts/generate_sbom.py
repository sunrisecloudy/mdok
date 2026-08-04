#!/usr/bin/env python3
"""Generate a dependency SBOM from the checked-in Cargo.lock file."""

from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path


def package_id(name: str, version: str, index: int) -> str:
    safe = "".join(character if character.isalnum() else "-" for character in name)
    return f"SPDXRef-Package-{safe}-{version.replace('.', '-')}-{index}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("dist/mdok.spdx.json"))
    args = parser.parse_args()

    lock_path = Path(__file__).resolve().parents[1] / "Cargo.lock"
    lock = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    packages = []
    for index, package in enumerate(lock.get("package", [])):
        name = package["name"]
        version = package["version"]
        entry = {
            "SPDXID": package_id(name, version, index),
            "name": name,
            "versionInfo": version,
            "downloadLocation": "NOASSERTION",
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "supplier": "NOASSERTION",
        }
        checksum = package.get("checksum")
        if checksum:
            entry["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        packages.append(entry)

    curl_checksum = (Path(__file__).resolve().parents[1] / "vendor/curl.sha256").read_text(
        encoding="utf-8"
    ).split()[0]
    packages.append(
        {
            "SPDXID": "SPDXRef-Package-curl-8-21-0",
            "name": "curl",
            "versionInfo": "8.21.0",
            "downloadLocation": "https://curl.se/download/curl-8.21.0.tar.xz",
            "licenseConcluded": "curl",
            "licenseDeclared": "curl",
            "supplier": "The curl project",
            "checksums": [{"algorithm": "SHA256", "checksumValue": curl_checksum}],
        }
    )

    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "mdok-cargo-dependencies",
        "documentNamespace": "https://mdok.dev/sbom/mdok-cargo-dependencies",
        "creationInfo": {
            "created": "1970-01-01T00:00:00Z",
            "creators": ["Tool: scripts/generate_sbom.py"],
        },
        "packages": packages,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {len(packages)} packages to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
