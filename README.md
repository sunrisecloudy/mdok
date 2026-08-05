# MDOK

MDOK turns ordinary Markdown into executable API workflow tests. Request blocks use copied `curl` syntax, checks and captures use standard JMESPath, and the CLI produces human, JSON, JSON Lines, and JUnit reports.

## Quick start

```sh
cargo build --release
cargo run -p mdok-cli -- lint mdok-prd/examples/auth-flow.md
cargo run -p mdok-cli -- plan mdok-prd/examples/auth-flow.md --var base_url=http://127.0.0.1:9800
cargo run -p mdok-cli -- test mdok-prd/examples/auth-flow.md --var base_url=http://127.0.0.1:9800
```

The bare form `mdok file.md` is an alias for `mdok test file.md`. Use `mdok-test-server --listen 127.0.0.1:0 --json-ready` for deterministic local HTTP fixtures.

## Project files

- `mdok.toml` controls language, execution limits, variables, and network/filesystem policy.
- `mdok-prd/` contains the product contract and the 495-fixture corpus.
- `crates/` contains the parser, template, restricted shell, curl, runtime, reporting, CLI, and fixture-server components.

MDOK performs whole-document planning before a request is sent. Interpolated values are data inside one argv element; they are never reparsed as shell source. HTTP/HTTPS and loopback-safe local testing are the default execution surface.

`curl` remains the default request fence. For repository-local agent tools,
MDOK also supports trusted-profile direct-process `exec` fences; command tests can
be stored as versioned Markdown and rerun over time. See
[docs/COMMAND_TESTS.md](docs/COMMAND_TESTS.md) for the policy and context
contract.

## Performance

The Criterion suite covers parser, Markdown, template, JMESPath, report, body
spill, normal/intense end-to-end, and one-shot versus reused-session cases:

```sh
make bench
python3 scripts/bench_performance.py --runs 10 --warmups 2
python3 scripts/audit_dependencies.py
```

The process harness builds the release CLI, runs deterministic loopback
fixtures, and records wall time plus peak RSS for normal and intense `lint`,
`plan`, and `test` workloads. See [docs/PERFORMANCE_CHECKLIST.md](docs/PERFORMANCE_CHECKLIST.md)
and [docs/DEPENDENCY_AUDIT.md](docs/DEPENDENCY_AUDIT.md) for targets and
interpretation.

## Release signing

`scripts/package.sh` produces deterministic unsigned local archives by default. Checksums are generated with the Python standard library, so the sidecars do not depend on `shasum` or `sha256sum`. A release operator can supply an Ed25519 PEM key through `MDOK_SIGNING_KEY`; signing writes base64 `.sig` sidecars and a signed `mdok-<version>-<target>.release.json` manifest. The verifier accepts the corresponding public PEM key (or the private key) through `MDOK_SIGNING_PUBLIC_KEY`.

```sh
openssl genpkey -algorithm ED25519 -out mdok-release-key.pem
openssl pkey -in mdok-release-key.pem -pubout -out mdok-release-key.pub.pem
MDOK_SIGNING_KEY="$PWD/mdok-release-key.pem" \
MDOK_SIGNING_PUBLIC_KEY="$PWD/mdok-release-key.pub.pem" \
MDOK_REQUIRE_SIGNATURE=1 \
./scripts/package.sh

python3 scripts/verify_release.py \
  --key mdok-release-key.pub.pem \
  --manifest dist/mdok-0.0.0-$(rustc -vV | awk '/host:/ { print $2 }').release.json
python3 scripts/release_smoke.py \
  --key mdok-release-key.pub.pem \
  --manifest dist/mdok-0.0.0-$(rustc -vV | awk '/host:/ { print $2 }').release.json
```

Signed packaging fails closed when the key is absent or the checkout is dirty. `MDOK_ALLOW_DIRTY_RELEASE=1` is an explicit local exception; its signed manifest and embedded provenance retain the `HEAD` revision, a dirty flag, and a hash of the complete porcelain status. `MDOK_RELEASE_SMOKE=1` verifies signatures before extracting and running the packaged binary. Unsigned local packaging remains available without that release gate.

The repository also provides a credential-free signed-release gate. It creates
an ephemeral Ed25519 key in a private temporary directory, packages and
verifies a signed release, checks the extracted binary and provenance bindings,
and removes the key on exit:

```sh
make release-smoke
```

For HTTPS/session portability, run the Tier-1 host matrix on each supported
platform and retain the JSON output:

```sh
make tls-matrix
```

See [docs/TLS_MATRIX.md](docs/TLS_MATRIX.md) for the target list and the four
cases that must pass on every host. The matrix is intentionally a manual
release gate; no CI workflow is added here.
