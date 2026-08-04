#!/usr/bin/env python3
"""Shared release checksum, signing, and verification helpers.

The signing format deliberately uses only Python's standard library plus the
system OpenSSL command.  Release keys are Ed25519 PEM keys.  Signatures are
base64-encoded so the sidecars remain portable text files, while the manifest
is deterministic and contains no timestamps or host paths.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Iterable


SCHEMA = "mdok.release.v1"
ALGORITHM = "Ed25519"
SIGNATURE_ENCODING = "base64"


class ReleaseSigningError(RuntimeError):
    """A fail-closed release signing or verification error."""


def _openssl() -> str:
    executable = shutil.which("openssl")
    if executable is None:
        raise ReleaseSigningError(
            "OpenSSL is required for release signing and verification; "
            "install OpenSSL 1.1.1+ and retry"
        )
    return executable


def _run(command: list[str]) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def _openssl_error(result: subprocess.CompletedProcess[bytes]) -> str:
    message = result.stderr.decode("utf-8", errors="replace").strip()
    return message or "OpenSSL command failed"


def _require_file(path: Path, description: str) -> Path:
    if not path.exists() or not path.is_file() or path.is_symlink():
        raise ReleaseSigningError(f"{description} is absent or not a regular file: {path}")
    return path


def _safe_name(name: str, description: str) -> str:
    path = Path(name)
    if not name or path.is_absolute() or path.name != name or ".." in path.parts:
        raise ReleaseSigningError(f"unsafe {description} name in release manifest: {name!r}")
    return name


def sha256_file(path: Path) -> str:
    _require_file(path, "release file")
    digest = hashlib.sha256()
    with path.open("rb") as payload:
        for chunk in iter(lambda: payload.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_source_state(root: Path) -> dict[str, Any]:
    """Return one immutable snapshot describing the source used for a build."""

    root = root.resolve()
    try:
        revision = subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            stderr=subprocess.PIPE,
            text=True,
        ).strip()
        status = subprocess.check_output(
            ["git", "-C", str(root), "status", "--porcelain=v1", "--untracked-files=all"],
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise ReleaseSigningError(f"could not snapshot Git source state for {root}: {error}") from error
    return {
        "revision": revision,
        "working_tree_dirty": bool(status),
        "working_tree_status_sha256": hashlib.sha256(status).hexdigest(),
    }


def read_source_state(path: Path) -> dict[str, Any]:
    _require_file(path, "source state")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseSigningError(f"could not read source state {path}: {error}") from error
    if (
        not isinstance(value, dict)
        or not isinstance(value.get("revision"), str)
        or not isinstance(value.get("working_tree_dirty"), bool)
        or not isinstance(value.get("working_tree_status_sha256"), str)
        or len(value["working_tree_status_sha256"]) != 64
    ):
        raise ReleaseSigningError(f"source state is malformed: {path}")
    return value


def write_checksum(artifact: Path) -> Path:
    artifact = _require_file(artifact, "artifact")
    checksum = artifact.with_name(f"{artifact.name}.sha256")
    checksum.write_text(f"{sha256_file(artifact)}  {artifact.name}\n", encoding="utf-8")
    return checksum


def _canonical_json(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


class OpenSSLSigner:
    """Ed25519 signer/verifier backed by a user-supplied PEM key."""

    def __init__(self, key_path: Path, temporary_directory: Path) -> None:
        self.key_path = _require_file(key_path.resolve(), "signing key")
        self.openssl = _openssl()
        self.temporary_directory = temporary_directory
        self.public_key = temporary_directory / "release-public-key.pem"
        self._normalize_public_key()
        self.key_fingerprint = self._fingerprint()
        self._require_ed25519()

    def _normalize_public_key(self) -> None:
        public_result = _run(
            [
                self.openssl,
                "pkey",
                "-pubin",
                "-in",
                str(self.key_path),
                "-out",
                str(self.public_key),
            ]
        )
        if public_result.returncode == 0:
            return

        private_result = _run(
            [
                self.openssl,
                "pkey",
                "-in",
                str(self.key_path),
                "-pubout",
                "-out",
                str(self.public_key),
            ]
        )
        if private_result.returncode != 0:
            raise ReleaseSigningError(
                f"could not read signing key {self.key_path}: "
                f"{_openssl_error(private_result)}"
            )

    def _fingerprint(self) -> str:
        result = _run(
            [
                self.openssl,
                "pkey",
                "-pubin",
                "-in",
                str(self.public_key),
                "-outform",
                "DER",
            ]
        )
        if result.returncode != 0:
            raise ReleaseSigningError(f"could not fingerprint signing key: {_openssl_error(result)}")
        return f"sha256:{hashlib.sha256(result.stdout).hexdigest()}"

    def _require_ed25519(self) -> None:
        result = _run(
            [
                self.openssl,
                "pkey",
                "-pubin",
                "-in",
                str(self.public_key),
                "-text",
                "-noout",
            ]
        )
        if result.returncode != 0 or b"ED25519" not in result.stdout.upper():
            detail = _openssl_error(result) if result.returncode else "key type is not Ed25519"
            raise ReleaseSigningError(f"release signing key must be an Ed25519 PEM key: {detail}")

    def require_private_key(self) -> None:
        result = _run(
            [self.openssl, "pkey", "-in", str(self.key_path), "-noout"]
        )
        if result.returncode != 0:
            raise ReleaseSigningError(
                f"release signing requires an Ed25519 private PEM key: {_openssl_error(result)}"
            )

    def _pkeyutl(self, operation: str, payload: Path, signature: Path) -> None:
        commands = []
        for raw_input in (True, False):
            command = [
                self.openssl,
                "pkeyutl",
                f"-{operation}",
                "-inkey",
                str(self.key_path if operation == "sign" else self.public_key),
                "-in",
                str(payload),
            ]
            if operation == "verify":
                command.extend(["-sigfile", str(signature), "-pubin"])
            else:
                command.extend(["-out", str(signature)])
            if raw_input:
                command.append("-rawin")
            commands.append(command)

        last_result: subprocess.CompletedProcess[bytes] | None = None
        for command in commands:
            result = _run(command)
            if result.returncode == 0:
                return
            last_result = result
        assert last_result is not None
        raise ReleaseSigningError(f"OpenSSL {operation} failed: {_openssl_error(last_result)}")

    def sign_bytes(self, payload: bytes) -> str:
        with tempfile.TemporaryDirectory(
            dir=self.temporary_directory, prefix="mdok-sign-"
        ) as directory:
            directory_path = Path(directory)
            payload_path = directory_path / "payload"
            signature_path = directory_path / "signature"
            payload_path.write_bytes(payload)
            self._pkeyutl("sign", payload_path, signature_path)
            return base64.b64encode(signature_path.read_bytes()).decode("ascii") + "\n"

    def verify_bytes(self, payload: bytes, encoded_signature: str) -> None:
        try:
            signature = base64.b64decode(encoded_signature.strip().encode("ascii"), validate=True)
        except (ValueError, UnicodeEncodeError) as error:
            raise ReleaseSigningError("signature is not valid base64") from error
        if not signature:
            raise ReleaseSigningError("signature is empty")

        with tempfile.TemporaryDirectory(
            dir=self.temporary_directory, prefix="mdok-verify-"
        ) as directory:
            directory_path = Path(directory)
            payload_path = directory_path / "payload"
            signature_path = directory_path / "signature"
            payload_path.write_bytes(payload)
            signature_path.write_bytes(signature)
            self._pkeyutl("verify", payload_path, signature_path)


def _signature_path(path: Path) -> Path:
    return path.with_name(f"{path.name}.sig")


def _write_signature(path: Path, signer: OpenSSLSigner) -> Path:
    signature_path = _signature_path(path)
    signature_path.write_text(signer.sign_bytes(path.read_bytes()), encoding="ascii")
    return signature_path


def _read_signature(path: Path) -> str:
    _require_file(path, "signature")
    try:
        return path.read_text(encoding="ascii")
    except UnicodeDecodeError as error:
        raise ReleaseSigningError(f"signature is not ASCII base64: {path}") from error


def _artifact_kind(path: Path) -> str:
    return "source" if path.name.endswith("-source.tar.gz") else "target"


def _manifest_artifact(
    artifact: Path,
    checksum: Path,
    artifact_signature: Path,
    checksum_signature: Path,
) -> dict[str, str]:
    return {
        "kind": _artifact_kind(artifact),
        "name": artifact.name,
        "sha256": sha256_file(artifact),
        "checksum_file": checksum.name,
        "checksum_sha256": sha256_file(checksum),
        "artifact_signature": artifact_signature.name,
        "checksum_signature": checksum_signature.name,
    }


def sign_release(
    *,
    key_path: Path,
    manifest_path: Path,
    version: str,
    target: str,
    artifacts: Iterable[Path],
    source_state_path: Path,
) -> Path:
    manifest_path = manifest_path.resolve()
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    artifact_paths = [path.resolve() for path in artifacts]
    if not artifact_paths:
        raise ReleaseSigningError("at least one release artifact is required")
    if len({path.name for path in artifact_paths}) != len(artifact_paths):
        raise ReleaseSigningError("release artifact names must be unique")

    for path in artifact_paths:
        if path.parent != manifest_path.parent:
            raise ReleaseSigningError(
                f"artifact must be next to the release manifest: {path}"
            )
        _safe_name(path.name, "artifact")

    with tempfile.TemporaryDirectory(prefix="mdok-release-sign-") as directory:
        signer = OpenSSLSigner(Path(key_path), Path(directory))
        signer.require_private_key()
        source_state = read_source_state(source_state_path)
        entries = []
        for artifact in sorted(artifact_paths, key=lambda path: path.name):
            checksum = write_checksum(artifact)
            artifact_signature = _write_signature(artifact, signer)
            checksum_signature = _write_signature(checksum, signer)
            entries.append(
                _manifest_artifact(artifact, checksum, artifact_signature, checksum_signature)
            )

        manifest = {
            "schema": SCHEMA,
            "product": "mdok",
            "version": version,
            "target": target,
            "source": source_state,
            "signing": {
                "algorithm": ALGORITHM,
                "signature_encoding": SIGNATURE_ENCODING,
                "key_fingerprint": signer.key_fingerprint,
                "manifest_signature": f"{manifest_path.name}.sig",
            },
            "artifacts": entries,
        }
        manifest_path.write_bytes(_canonical_json(manifest))
        _write_signature(manifest_path, signer)

    print(f"Signed {len(entries)} release artifact(s) in {manifest_path.parent}")
    return manifest_path


def _load_manifest(manifest_path: Path) -> dict[str, Any]:
    _require_file(manifest_path, "release manifest")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseSigningError(f"could not read release manifest {manifest_path}: {error}") from error
    if not isinstance(manifest, dict):
        raise ReleaseSigningError("release manifest must be a JSON object")
    return manifest


def _manifest_path(base: Path, name: str, description: str) -> Path:
    safe_name = _safe_name(name, description)
    path = base / safe_name
    if path.parent != base:
        raise ReleaseSigningError(f"release {description} escapes the manifest directory: {name}")
    return path


def _verify_checksum(artifact: Path, checksum: Path, expected: str) -> None:
    actual = sha256_file(artifact)
    if actual != expected:
        raise ReleaseSigningError(
            f"artifact checksum mismatch for {artifact.name}: expected {expected}, got {actual}"
        )
    lines = checksum.read_text(encoding="utf-8").splitlines()
    if lines != [f"{expected}  {artifact.name}"]:
        raise ReleaseSigningError(f"checksum file does not exactly bind {artifact.name}: {checksum}")


def verify_release(*, key_path: Path, manifest_path: Path) -> dict[str, Any]:
    manifest_path = manifest_path.resolve()
    manifest = _load_manifest(manifest_path)
    with tempfile.TemporaryDirectory(prefix="mdok-release-verify-") as directory:
        signer = OpenSSLSigner(Path(key_path), Path(directory))
        signing = manifest.get("signing")
        if not isinstance(signing, dict):
            raise ReleaseSigningError("release manifest signing metadata is invalid")
        manifest_signature_name = signing.get("manifest_signature")
        if not isinstance(manifest_signature_name, str):
            raise ReleaseSigningError("release manifest is missing signing.manifest_signature")
        if manifest_signature_name != f"{manifest_path.name}.sig":
            raise ReleaseSigningError("manifest signature must be the adjacent .sig sidecar")
        manifest_signature = _manifest_path(
            manifest_path.parent, manifest_signature_name, "manifest signature"
        )
        signer.verify_bytes(manifest_path.read_bytes(), _read_signature(manifest_signature))

        if manifest.get("schema") != SCHEMA or manifest.get("product") != "mdok":
            raise ReleaseSigningError("unsupported or missing release manifest schema")
        if signing.get("algorithm") != ALGORITHM or signing.get("signature_encoding") != SIGNATURE_ENCODING:
            raise ReleaseSigningError("release manifest signing algorithm or encoding is unsupported")
        if signing.get("key_fingerprint") != signer.key_fingerprint:
            raise ReleaseSigningError("release signing key fingerprint does not match manifest")

        source = manifest.get("source")
        if (
            not isinstance(source, dict)
            or not isinstance(source.get("revision"), str)
            or not isinstance(source.get("working_tree_dirty"), bool)
            or not isinstance(source.get("working_tree_status_sha256"), str)
            or len(source["working_tree_status_sha256"]) != 64
        ):
            raise ReleaseSigningError("release manifest source state is invalid")

        artifacts = manifest.get("artifacts")
        if not isinstance(artifacts, list) or not artifacts:
            raise ReleaseSigningError("release manifest contains no artifacts")
        names: set[str] = set()
        sidecar_names: set[str] = set()
        for entry in artifacts:
            if not isinstance(entry, dict):
                raise ReleaseSigningError("release manifest artifact entry is invalid")
            required = (
                "kind",
                "name",
                "sha256",
                "checksum_file",
                "checksum_sha256",
                "artifact_signature",
                "checksum_signature",
            )
            if any(key not in entry for key in required):
                raise ReleaseSigningError("release manifest artifact entry is incomplete")
            name = entry["name"]
            if (
                not isinstance(name, str)
                or name in names
                or entry["kind"] not in ("source", "target")
                or not isinstance(entry["sha256"], str)
                or len(entry["sha256"]) != 64
                or not isinstance(entry["checksum_sha256"], str)
                or len(entry["checksum_sha256"]) != 64
            ):
                raise ReleaseSigningError("release manifest artifact names are invalid or duplicated")
            names.add(name)
            artifact = _manifest_path(manifest_path.parent, name, "artifact")
            sidecar_values = (
                entry["checksum_file"],
                entry["artifact_signature"],
                entry["checksum_signature"],
            )
            if any(not isinstance(value, str) for value in sidecar_values):
                raise ReleaseSigningError(f"release manifest sidecars are invalid for {name}")
            if sidecar_values != (
                f"{name}.sha256",
                f"{name}.sig",
                f"{name}.sha256.sig",
            ):
                raise ReleaseSigningError(f"release manifest sidecars do not match {name}")
            if any(value in names or value in sidecar_names for value in sidecar_values):
                raise ReleaseSigningError(f"release manifest sidecars collide for {name}")
            sidecar_names.update(sidecar_values)
            checksum = _manifest_path(manifest_path.parent, sidecar_values[0], "checksum file")
            artifact_signature = _manifest_path(
                manifest_path.parent, sidecar_values[1], "artifact signature"
            )
            checksum_signature = _manifest_path(
                manifest_path.parent, sidecar_values[2], "checksum signature"
            )
            _require_file(artifact, "artifact")
            _require_file(checksum, "checksum file")
            _verify_checksum(artifact, checksum, entry["sha256"])
            if sha256_file(checksum) != entry["checksum_sha256"]:
                raise ReleaseSigningError(f"checksum metadata mismatch for {name}")
            signer.verify_bytes(artifact.read_bytes(), _read_signature(artifact_signature))
            signer.verify_bytes(checksum.read_bytes(), _read_signature(checksum_signature))

    print(f"Verified signed release manifest {manifest_path}")
    return manifest


def _checksums_command(artifacts: list[Path]) -> int:
    if not artifacts:
        raise ReleaseSigningError("at least one artifact is required")
    for artifact in artifacts:
        checksum = write_checksum(artifact)
        print(f"Wrote {checksum}")
    return 0


def _source_state_command(root: Path, output: Path | None, field: str | None) -> int:
    state = git_source_state(root)
    if output is not None:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(_canonical_json(state))
    if field is not None:
        print(json.dumps(state[field]))
    elif output is None:
        print(json.dumps(state, indent=2, sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    checksums = subparsers.add_parser("checksums", help="write deterministic SHA-256 sidecars")
    checksums.add_argument("--artifact", type=Path, action="append", required=True)
    source_state = subparsers.add_parser("source-state", help="snapshot Git revision and dirty state")
    source_state.add_argument("--root", type=Path, required=True)
    source_state.add_argument("--output", type=Path)
    source_state.add_argument(
        "--field",
        choices=("revision", "working_tree_dirty", "working_tree_status_sha256"),
    )
    args = parser.parse_args()
    try:
        if args.command == "checksums":
            return _checksums_command(args.artifact)
        if args.command == "source-state":
            return _source_state_command(args.root, args.output, args.field)
    except ReleaseSigningError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
