#!/usr/bin/env python3
"""Verify a signed MDOK release, then smoke the target archive."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import stat
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any

from release_signing import ReleaseSigningError, sha256_file, verify_release


def _safe_member(root: Path, name: str) -> Path:
    relative = PurePosixPath(name)
    if relative.is_absolute() or ".." in relative.parts:
        raise ReleaseSigningError(f"unsafe archive member: {name}")
    destination = (root / Path(*relative.parts)).resolve()
    if root.resolve() not in destination.parents and destination != root.resolve():
        raise ReleaseSigningError(f"archive member escapes extraction directory: {name}")
    return destination


def _extract(archive: Path, destination: Path) -> None:
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.infolist():
                target = _safe_member(destination, member.filename)
                if member.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                with bundle.open(member) as source, target.open("wb") as output:
                    shutil.copyfileobj(source, output)
        return

    with tarfile.open(archive, mode="r:*") as bundle:
        for member in bundle.getmembers():
            target = _safe_member(destination, member.name)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            if not member.isfile():
                raise ReleaseSigningError(f"unsupported archive member type: {member.name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            source = bundle.extractfile(member)
            if source is None:
                raise ReleaseSigningError(f"could not read archive member: {member.name}")
            with source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
            target.chmod(stat.S_IMODE(member.mode) or 0o644)


def _find_binary(root: Path, binary_name: str) -> Path:
    binaries = [path for path in root.rglob(binary_name) if path.is_file()]
    if not binaries:
        raise ReleaseSigningError(f"release archive does not contain {binary_name}")
    if len(binaries) != 1:
        raise ReleaseSigningError(
            f"release archive contains {len(binaries)} copies of {binary_name}; expected exactly one"
        )
    return binaries[0]


def _run_binary(binary: Path, root: Path) -> None:
    if not os.access(binary, os.X_OK):
        binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
    result = subprocess.run(
        [str(binary), "version", "--json"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise ReleaseSigningError(
            f"release binary smoke failed with exit {result.returncode}: {result.stderr.strip()}"
        )
    try:
        version = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ReleaseSigningError("release binary did not emit JSON version metadata") from error
    for field in ("mdok_version", "curl_version", "libcurl", "tls", "features"):
        if field not in version:
            raise ReleaseSigningError(f"release version metadata is missing {field}")


def _required_string(value: Any, description: str) -> str:
    if not isinstance(value, str) or not value:
        raise ReleaseSigningError(f"provenance binding is missing a non-empty {description}")
    return value


def _required_sha256(value: Any, description: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ReleaseSigningError(f"provenance binding contains an invalid {description}")
    return value


def _load_provenance(root: Path) -> dict[str, Any]:
    candidates = [path for path in root.rglob("mdok.provenance.json") if path.is_file()]
    if len(candidates) != 1:
        raise ReleaseSigningError(
            "release archive must contain exactly one regular mdok.provenance.json file"
        )
    try:
        value = json.loads(candidates[0].read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseSigningError(
            f"could not read extracted provenance {candidates[0]}: {error}"
        ) from error
    if not isinstance(value, dict):
        raise ReleaseSigningError("extracted provenance must be a JSON object")
    return value


def _verify_provenance(root: Path, manifest: dict[str, Any], binary: Path) -> None:
    provenance = _load_provenance(root)
    if provenance.get("_type") != "https://in-toto.io/Statement/v1":
        raise ReleaseSigningError("extracted provenance has an unsupported statement type")
    if provenance.get("predicateType") != "https://slsa.dev/provenance/v1":
        raise ReleaseSigningError("extracted provenance has an unsupported predicate type")

    subjects = provenance.get("subject")
    if not isinstance(subjects, list) or len(subjects) != 1 or not isinstance(subjects[0], dict):
        raise ReleaseSigningError("extracted provenance must contain exactly one subject")
    subject = subjects[0]
    if subject.get("name") != binary.name:
        raise ReleaseSigningError(
            f"provenance subject does not identify the packaged binary {binary.name}"
        )
    digest = subject.get("digest")
    if not isinstance(digest, dict):
        raise ReleaseSigningError("extracted provenance subject digest is invalid")
    subject_digest = _required_sha256(digest.get("sha256"), "subject digest")
    binary_digest = sha256_file(binary)
    if subject_digest != binary_digest:
        raise ReleaseSigningError(
            f"provenance subject digest does not match {binary.name}: "
            f"expected {binary_digest}, got {subject_digest}"
        )

    predicate = provenance.get("predicate")
    if not isinstance(predicate, dict):
        raise ReleaseSigningError("extracted provenance predicate is invalid")
    build_definition = predicate.get("buildDefinition")
    if not isinstance(build_definition, dict):
        raise ReleaseSigningError("extracted provenance build definition is invalid")
    if build_definition.get("buildType") != "https://mdok.dev/build/package/v1":
        raise ReleaseSigningError("extracted provenance build type is unsupported")
    parameters = build_definition.get("externalParameters")
    if not isinstance(parameters, dict):
        raise ReleaseSigningError("extracted provenance external parameters are invalid")

    target = _required_string(manifest.get("target"), "signed manifest target")
    source = manifest.get("source")
    if not isinstance(source, dict):
        raise ReleaseSigningError("signed manifest source state is invalid")
    source_revision = _required_string(source.get("revision"), "signed manifest source revision")

    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list):
        raise ReleaseSigningError("signed manifest artifacts are invalid")
    source_entries = [
        entry for entry in artifacts if isinstance(entry, dict) and entry.get("kind") == "source"
    ]
    if len(source_entries) != 1:
        raise ReleaseSigningError(
            "signed manifest must contain exactly one source archive for provenance binding"
        )
    source_archive_digest = _required_sha256(
        source_entries[0].get("sha256"), "signed manifest source archive digest"
    )

    expected_bindings = {
        "target": target,
        "source_revision": source_revision,
        "source_archive_revision": source_revision,
        "source_archive_sha256": source_archive_digest,
    }
    for field, expected in expected_bindings.items():
        actual = parameters.get(field)
        if actual != expected:
            raise ReleaseSigningError(
                f"provenance {field} does not match the signed release manifest: "
                f"expected {expected}, got {actual!r}"
            )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key", type=Path, required=True, help="trusted Ed25519 PEM public or private key")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="allow a manifest explicitly marked as built from a dirty checkout",
    )
    args = parser.parse_args()
    try:
        manifest = verify_release(key_path=args.key, manifest_path=args.manifest)
        if manifest["source"]["working_tree_dirty"] and not args.allow_dirty:
            raise ReleaseSigningError(
                "release smoke refuses a manifest from a dirty checkout; "
                "pass --allow-dirty only for an explicit local exception"
            )
        target_entries = [entry for entry in manifest["artifacts"] if entry["kind"] == "target"]
        if len(target_entries) != 1:
            raise ReleaseSigningError("release smoke requires exactly one target artifact")
        archive = args.manifest.resolve().parent / target_entries[0]["name"]
        with tempfile.TemporaryDirectory(prefix="mdok-release-smoke-") as directory:
            extraction_root = Path(directory)
            _extract(archive, extraction_root)
            required = ("LICENSE", "THIRD_PARTY.md", "COPYING", "mdok.spdx.json", "mdok.provenance.json")
            for name in required:
                if next(extraction_root.rglob(name), None) is None:
                    raise ReleaseSigningError(f"release archive is missing {name}")
            if not list(extraction_root.rglob("patches/curl/*.patch")):
                raise ReleaseSigningError("release archive is missing the curl patch notice")
            binary = _find_binary(
                extraction_root, "mdok.exe" if archive.name.endswith(".zip") else "mdok"
            )
            _verify_provenance(extraction_root, manifest, binary)
            _run_binary(binary, extraction_root)
    except (OSError, ReleaseSigningError, subprocess.SubprocessError) as error:
        parser.error(str(error))
    print(f"Release signature verification and smoke passed for {args.manifest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
