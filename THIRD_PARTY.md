# Third-party notices

MDOK is distributed under the MIT license.

The workspace uses the following permissively licensed components:

- Rust standard library and Cargo ecosystem crates declared in `Cargo.toml`.
- curl/libcurl compatibility metadata under `vendor/`; a release build must include the upstream curl notices and the verified source checksum.

`Cargo.lock` is generated for release builds. Dependency license and vulnerability checks are part of the release checklist in `mdok-prd/docs/19-release-and-supply-chain.md`.

