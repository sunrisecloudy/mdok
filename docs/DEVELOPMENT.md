# MDOK developer and maintainer guide

This document collects repository, testing, performance, and release details
that do not belong in the first-time-user README.

## Repository layout

- `mdok.toml` — language, execution, variable, and network/filesystem policy.
- `mdok-prd/` — product contract, examples, and the 495-fixture corpus.
- `crates/` — the Rust reference implementation: parser, template, restricted
  shell, curl, runtime, reporting, CLI, and fixture-server components.
- `go/` — the Go implementation distributed via Homebrew (`go/cmd/mdok`) and
  the fixture server (`go/cmd/test-server`). Pure Go, no cgo.
- `tests/e2e/` — focused Markdown workflows and the combined E2E workflow.
- `skills/mdok/SKILL.md` — reusable instructions for coding agents.

MDOK performs whole-document planning before a request is sent. Interpolated
values remain data inside one argv element and are never reparsed as shell
source. HTTP/HTTPS and loopback-safe local testing are the default execution
surface.

`curl` remains the default request fence. Repository-local agent tools may also
use trusted-profile `exec` fences; see [COMMAND_TESTS.md](COMMAND_TESTS.md).

## Two implementations

The Rust workspace in `crates/` is the reference implementation and carries
the full feature set (MCP server, record/replay, import, plan/list, exec
fences, QuickJS sandbox). The Go module in `go/` is the distributed binary
(Homebrew, release archives) and implements `lint`, `test`, and `version`
over the ported feature matrix. The Go port is gated by a differential
parity suite (`docs/GO_PORT_TEST_PLAN.md` records the full test-asset map;
`docs/GO_PORT_ESTIMATE.md` the porting roadmap).

Every change to either implementation that shifts exit codes, document or
step statuses, or diagnostic codes anywhere in the generated matrix must
keep `make parity` green, or the divergence is intentional and documented.

## Tests and checks

```sh
# Rust reference
cargo test --locked --workspace
make e2e-md
python3 mdok-prd/scripts/validate_corpus.py

# Go implementation
cd go && go vet ./... && go test ./...
zsh go/build.sh                       # from repo root: builds go/bin/*
make parity                           # 891-case Rust-vs-Go differential suite
make mcp-conformance                  # 22-case MCP wire-protocol suite (Rust)
make golden                           # normalized-output parity gate
python3 scripts/run_md_e2e.py \
  --binary go/bin/mdok --server go/bin/test-server --skip-build
```

The E2E runner starts a deterministic loopback fixture and executes every file
listed in `tests/e2e/manifest.txt`. The PRD corpus validator checks all 497
fixtures and its manifest. The parity suite runs both binaries against the
Go fixture server and compares normalized outcomes case by case.

## Go release flow

1. Bump `mdokVersion` in `go/cmd/mdok/main.go`.
2. Verify: `go test ./...`, e2e with both Go binaries, `make parity`.
3. Cross-compile and package (`mdok-<version>-<target>.tar.gz` per platform
   plus `.sha256` sidecars), smoke-test the local artifact through the E2E
   suite.
4. Tag the version, push, and create the GitHub release with the archives.
5. Update `sunrisecloudy/homebrew-tap` `Formula/mdok.rb` (version, URLs,
   digests), then verify `brew install sunrisecloudy/tap/mdok` and run the
   installed binary through the E2E suite.

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
