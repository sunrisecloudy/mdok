# Third-party notices

MDOK is distributed under the MIT license.

The workspace uses the following permissively licensed components:

- Rust standard library and Cargo ecosystem crates declared in `Cargo.toml`.
- curl/libcurl 8.21.0 under `vendor/curl/`, including the upstream `COPYING` notice, the maintained MDOK patch series under `vendor/patches/curl/`, and the verified source checksum in `vendor/curl.sha256`. Release archives include that notice alongside `LICENSE` and this file.

`Cargo.lock` is checked in. `scripts/generate_sbom.py` produces an SPDX 2.3 inventory from it, and `scripts/package.sh` includes that inventory, deterministic in-toto/SLSA-style provenance, and this notice in a reproducible archive. Dependency license and vulnerability checks remain part of the release checklist in `mdok-prd/docs/19-release-and-supply-chain.md`.
