# Third-party notices

MDOK is distributed under the MIT license.

The workspace uses the following permissively licensed components:

- Rust standard library and Cargo ecosystem crates declared in `Cargo.toml`.
- curl/libcurl 8.21.0 under `vendor/curl/`, including the upstream `COPYING` notice, the maintained MDOK patch series under `vendor/patches/curl/`, and the verified source checksum in `vendor/curl.sha256`. Release archives include that notice alongside `LICENSE` and this file.

`Cargo.lock` is checked in. `scripts/generate_sbom.py` produces an SPDX 2.3 inventory from it, and `scripts/package.sh` includes that inventory, deterministic in-toto/SLSA-style provenance, and this notice in a reproducible archive. Dependency license and vulnerability checks remain part of the release checklist in `mdok-prd/docs/19-release-and-supply-chain.md`.

Signed release output uses the schema in `specs/release-manifest.schema.json`. The manifest binds each archive and checksum sidecar to Ed25519 signatures and records the exact Git revision plus dirty-working-tree state used for the build. The public key is always supplied out of band to verification; a key embedded in an untrusted release directory is not treated as a trust anchor.
