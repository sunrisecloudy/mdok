# TLS and session portability matrix

`scripts/run_tls_matrix.py` is the portable, CI-independent Tier-1 runtime
gate for HTTPS verification, TLS policy, and document-scoped session reuse.
Run it on the host that owns the target binary; the `--target` value is a
Rust target triple and is recorded in the JSON result.

```sh
python3 scripts/run_tls_matrix.py \
  --target "$(rustc -vV | awk '/^host:/ { print $2 }')" \
  --output "target/tls-matrix-$(rustc -vV | awk '/^host:/ { print $2 }').json"
```

The runner builds the release CLI and fixture server unless `--skip-build` is
specified. It starts the fixture with a dynamically assigned HTTP, HTTPS, and
proxy port, reads its JSON readiness record without an unbounded pipe read,
and always terminates the fixture. The fixture uses a separate self-signed CA
and loopback server leaf, so strict TLS implementations receive a real chain.

Each run must pass all four cases:

1. verified custom-CA HTTPS with two sequential same-origin requests;
2. wrong-CA rejection with `MDOK-E602`;
3. default denial of `--insecure` with `MDOK-E602`; and
4. explicitly allowed local `--insecure` through the native path, again with
   two sequential requests.

Tier-1 targets from the portability contract are:

| Target | Host required for runtime evidence |
| --- | --- |
| `aarch64-apple-darwin` | macOS arm64 |
| `x86_64-apple-darwin` | macOS x86_64 |
| `x86_64-unknown-linux-gnu` | Linux x86_64 glibc |
| `aarch64-unknown-linux-gnu` | Linux aarch64 glibc |
| `x86_64-pc-windows-msvc` | Windows x86_64 MSVC |

The current local evidence is a four-case pass on `aarch64-apple-darwin`.
That result does not substitute for the other host runs: each remaining
target must execute the same command and retain its JSON result before a
release is called all-platform validated. This is deliberately a manual
release gate, not a CI implementation.

