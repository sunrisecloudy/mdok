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

from release_signing import ReleaseSigningError, verify_release


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


def _run_binary(root: Path, binary_name: str) -> None:
    binary = next(root.rglob(binary_name), None)
    if binary is None or not binary.is_file():
        raise ReleaseSigningError(f"release archive does not contain {binary_name}")
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
            _run_binary(extraction_root, "mdok.exe" if archive.name.endswith(".zip") else "mdok")
    except (OSError, ReleaseSigningError, subprocess.SubprocessError) as error:
        parser.error(str(error))
    print(f"Release signature verification and smoke passed for {args.manifest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
