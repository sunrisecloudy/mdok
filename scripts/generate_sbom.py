#!/usr/bin/env python3
"""Generate a license-aware SPDX SBOM for the checked-in MDOK build graph."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DISALLOWED_LICENSE = re.compile(r"(?:^|[^A-Za-z])(A?GPL)(?:[-+]|$)", re.IGNORECASE)


def package_id(name: str, version: str, source: str | None, index: int) -> str:
    safe_name = "".join(character if character.isalnum() else "-" for character in name)
    safe_source = "local" if source is None else "registry"
    return f"SPDXRef-Package-{safe_name}-{version.replace('.', '-')}-{safe_source}-{index}"


def cargo_metadata() -> dict[str, Any]:
    completed = subprocess.run(
        [
            "cargo",
            "metadata",
            "--offline",
            "--locked",
            "--format-version",
            "1",
        ],
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode:
        raise RuntimeError(
            "cargo metadata failed; SBOM generation is fail-closed:\n"
            + completed.stderr.strip()
        )
    return json.loads(completed.stdout)


def lock_checksums() -> dict[tuple[str, str, str | None], str]:
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    return {
        (package["name"], package["version"], package.get("source")): package["checksum"]
        for package in lock.get("package", [])
        if package.get("checksum")
    }


def download_location(package: dict[str, Any]) -> str:
    source = package.get("source")
    if source is None:
        return "NOASSERTION"
    if "crates.io-index" in source:
        return f"https://crates.io/crates/{package['name']}/{package['version']}"
    return source


def package_entry(
    package: dict[str, Any],
    checksums: dict[tuple[str, str, str | None], str],
    index: int,
) -> dict[str, Any]:
    name = package["name"]
    version = package["version"]
    source = package.get("source")
    declared = package.get("license") or "NOASSERTION"
    entry: dict[str, Any] = {
        "SPDXID": package_id(name, version, source, index),
        "name": name,
        "versionInfo": version,
        "downloadLocation": download_location(package),
        "licenseConcluded": declared,
        "licenseDeclared": declared,
        "supplier": "NOASSERTION",
        "externalRefs": [
            {
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": f"pkg:cargo/{name}@{version}",
            }
        ],
    }
    checksum = checksums.get((name, version, source))
    if checksum:
        entry["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
    return entry


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("dist/mdok.spdx.json"))
    parser.add_argument(
        "--allow-unresolved-licenses",
        action="store_true",
        help="permit Cargo packages without license metadata; release packaging should not use this",
    )
    args = parser.parse_args()

    metadata = cargo_metadata()
    packages = sorted(
        metadata.get("packages", []),
        key=lambda package: (package["name"], package["version"], package.get("source") or ""),
    )
    checksums = lock_checksums()
    unresolved = [package["name"] for package in packages if not package.get("license")]
    disallowed = [
        f"{package['name']} ({package.get('license')})"
        for package in packages
        if package.get("license") and DISALLOWED_LICENSE.search(package["license"])
    ]
    if unresolved and not args.allow_unresolved_licenses:
        raise SystemExit(
            "SBOM license metadata is incomplete for: " + ", ".join(sorted(unresolved))
        )
    if disallowed:
        raise SystemExit("disallowed GPL-family license expression(s): " + ", ".join(disallowed))

    entries = [package_entry(package, checksums, index) for index, package in enumerate(packages)]
    package_ids_by_name: dict[str, list[str]] = {}
    for entry in entries:
        package_ids_by_name.setdefault(entry["name"], []).append(entry["SPDXID"])
    relationships = [
        {
            "spdxElementId": "SPDXRef-DOCUMENT",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": entry["SPDXID"],
        }
        for entry in entries
    ]
    for package in packages:
        source_id = package_ids_by_name[package["name"]][0]
        for dependency in package.get("dependencies", []):
            targets = package_ids_by_name.get(dependency["name"], [])
            if targets:
                relationships.append(
                    {
                        "spdxElementId": source_id,
                        "relationshipType": "DEPENDS_ON",
                        "relatedSpdxElement": targets[0],
                    }
                )

    curl_checksum = (ROOT / "vendor/curl.sha256").read_text(encoding="utf-8").split()[0]
    curl_id = "SPDXRef-Package-curl-8-21-0-vendored"
    entries.append(
        {
            "SPDXID": curl_id,
            "name": "curl",
            "versionInfo": "8.21.0",
            "downloadLocation": "https://curl.se/download/curl-8.21.0.tar.xz",
            "licenseConcluded": "curl",
            "licenseDeclared": "curl",
            "supplier": "The curl project",
            "checksums": [{"algorithm": "SHA256", "checksumValue": curl_checksum}],
            "externalRefs": [
                {
                    "referenceCategory": "PACKAGE-MANAGER",
                    "referenceType": "purl",
                    "referenceLocator": "pkg:generic/curl@8.21.0",
                }
            ],
        }
    )
    relationships.append(
        {
            "spdxElementId": "SPDXRef-DOCUMENT",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": curl_id,
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
        "packages": entries,
        "relationships": relationships,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {len(entries)} packages and {len(relationships)} relationships to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
