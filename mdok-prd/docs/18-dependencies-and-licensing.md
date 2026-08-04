# 18. Dependencies and Licensing

## 18.1 Planned primary dependencies

- Comrak for CommonMark/GFM parsing and AST/source positions.
- Tree-sitter and Tree-sitter Bash for restricted shell syntax parsing.
- A Rust JMESPath implementation plus the upstream compliance suite.
- TOML/Serde for configuration and typed values.
- Clap for the CLI.
- Tokio or an equivalent runtime only if it materially simplifies multi-handle polling and fixture-server implementation; avoid async complexity in parser crates.
- curl source and libcurl under curl's permissive license.

Exact dependency versions are pinned at implementation start and updated through review. The PRD does not assume that a crate's API is stable merely because its semver is stable.

## 18.2 License recommendation

Apache-2.0 OR MIT is recommended for MDOK. Curl notices and the curl license must be included in binary/source distributions. Generated SBOM and `THIRD_PARTY.md` identify bundled components and enabled features.

## 18.3 Dependency policy

- Prefer mature parsers and standards implementations over custom regex/line parsers.
- Minimize unsafe and transitive build-time execution.
- No GPL dependency in the shipped binary unless the project's license strategy explicitly accepts it.
- Audit parsing, TLS, serialization, and FFI dependencies more strictly than convenience libraries.
