# MDOK developer and maintainer guide

This document collects repository, testing, performance, and release details
that do not belong in the first-time-user README.

## Repository layout

- `mdok.toml` — language, execution, variable, and network/filesystem policy.
- `mdok-prd/` — product contract, examples, and the 495-fixture corpus.
- `crates/` — parser, template, restricted shell, curl, runtime, reporting,
  CLI, and fixture-server components.
- `tests/e2e/` — focused Markdown workflows and the combined E2E workflow.
- `skills/mdok/SKILL.md` — reusable instructions for coding agents.

MDOK performs whole-document planning before a request is sent. Interpolated
values remain data inside one argv element and are never reparsed as shell
source. HTTP/HTTPS and loopback-safe local testing are the default execution
surface.

`curl` remains the default request fence. Repository-local agent tools may also
use trusted-profile `exec` fences; see [COMMAND_TESTS.md](COMMAND_TESTS.md).

## Tests and checks

```sh
cargo test --locked --workspace
make e2e-md
python3 mdok-prd/scripts/validate_corpus.py
```

The E2E runner starts a deterministic loopback fixture and executes every file
listed in `tests/e2e/manifest.txt`. The PRD corpus validator checks all 495
fixtures and its manifest.

## Performance

The Criterion suite covers parser, Markdown, template, JMESPath, reporting,
body spill, normal/intense end-to-end, and one-shot versus reused-session
cases:

```sh
make bench
python3 scripts/bench_performance.py --runs 10 --warmups 2
python3 scripts/audit_dependencies.py
```

See [PERFORMANCE_CHECKLIST.md](PERFORMANCE_CHECKLIST.md) and
[DEPENDENCY_AUDIT.md](DEPENDENCY_AUDIT.md) for targets and interpretation.

## Release signing

`scripts/package.sh` produces deterministic unsigned local archives. A release
operator can supply an Ed25519 PEM key through `MDOK_SIGNING_KEY`; signed
manifests retain provenance and checksums. Verify a signed release with:

```sh
version=$(awk -F '"' '/^version = / { print $2; exit }' crates/mdok-cli/Cargo.toml)
target=$(rustc -vV | awk '/^host:/ { print $2 }')
python3 scripts/verify_release.py \
  --key mdok-release-key.pub.pem \
  --manifest "dist/mdok-$version-$target.release.json"
python3 scripts/release_smoke.py \
  --key mdok-release-key.pub.pem \
  --manifest "dist/mdok-$version-$target.release.json"
```

The credential-free local gate creates an ephemeral key and removes it after
verification:

```sh
make release-smoke
```

Signed packaging fails closed when a key is absent or the checkout is dirty.
`MDOK_ALLOW_DIRTY_RELEASE=1` is an explicit local exception; unsigned local
packaging remains available without that release gate.

## TLS and portability

Run the HTTPS/session portability matrix on each supported host:

```sh
make tls-matrix
```

See [TLS_MATRIX.md](TLS_MATRIX.md) for the target list and the required cases.
