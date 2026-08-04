# 14. Portability and Build

## 14.1 Supported targets

Tier 1 target set:

- macOS arm64 and x86_64;
- Linux x86_64 and aarch64 using glibc;
- Windows x86_64 MSVC.

Musl static builds are Tier 2 because static TLS/libcurl dependency composition is more complex. FreeBSD and other Unix targets are community-supported initially.

## 14.2 Bundled curl

Release artifacts bundle a known curl/libcurl build so behavior does not depend on the host's curl executable. TLS backend choices should match platform expectations where possible:

- macOS: Secure Transport/SecTrust-compatible build or OpenSSL/rustls after validation;
- Windows: Schannel by default;
- Linux: OpenSSL or rustls, chosen and documented per artifact.

Feature matrices are printed by `mdok version --json`.

## 14.3 Local development prerequisites

- Rust stable toolchain pinned by `rust-toolchain.toml`;
- C11 compiler;
- CMake and Ninja;
- Python 3 for generation/test scripts;
- platform TLS/build dependencies when not using fully vendored dependencies.

## 14.4 Reproducibility

Release builds use Cargo lockfiles, pinned curl source and checksum, pinned patch series, explicit CMake options, and recorded compiler versions. Build metadata is embedded in `mdok version` without making binaries nondeterministic where reproducibility is enabled.
