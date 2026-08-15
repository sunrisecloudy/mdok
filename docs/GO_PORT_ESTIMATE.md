# Go Port Estimate

Effort estimate and sequencing for porting mdok to Go, assuming the
**hybrid architecture** agreed on 2026-08-15:

- HTTP execution: `net/http` replaces both the native curl bridge and the
  reqwest fallback path (the dual-path design collapses into one engine).
- curl **argv parsing and policy** are re-implemented in Go, gated by the
  655-case differential against pinned curl 8.21 (`specs/curl-option-policy.csv`
  is the classification source of truth).
- QuickJS is **retained via cgo** to preserve the sandbox contract
  (memory/stack limits, deadline interrupt); the Rust wrapper around rquickjs
  is re-built on a custom cgo binding.
- Everything else (CLI, MCP, report, runtime, markdown, templates, postman,
  fixtures) is a from-scratch Go implementation validated by the portable
  suites catalogued in `docs/GO_PORT_TEST_PLAN.md`.

Estimates are dev-days for a senior developer fluent in both Rust and Go,
**including** re-authoring the unit-test semantics (the 204 white-box Rust
tests do not translate). They exclude product changes: this is behavior
parity only.

## Per-crate effort

| # | Rust crate (src LOC) | Go package(s) | Difficulty | Dev-days |
|---|---|---|---|---|
| 1 | mdok-core (954) | `core` — types, error codes, names | trivial | 3–5 |
| 2 | mdok-template (575) | `template` — mini-engine + filters | trivial | 3–4 |
| 3 | mdok-shell (529) | `shell` — restricted argv parser | moderate | 4–6 |
| 4 | mdok-markdown (775) | `markdown` — goldmark walk + limits | moderate | 5–8 |
| 5 | mdok-jmespath (194) | `jmespath` — go-jmespath wrap + parity fixes | moderate | 3–5 |
| 6 | mdok-runtime (690) | `runtime` — sequential executor | trivial | 4–6 |
| 7 | mdok-report (2,102) | `report` — schemas, redaction, JUnit, atomic writes | moderate | 8–12 |
| 8 | mdok-command (846) | `command` — process groups, caps, reaper | moderate | 5–8 |
| 9 | mdok-curl (3,755) | `curlplan` + `httpx` — argv parser, policy engine, net/http engine, artifacts | **hard** | 20–30 |
| 10 | mdok-quickjs (2,256 + 374 KB JS) | `quickjs` — cgo binding + pm facade + secrets/transcript | **hard** | 15–25 |
| 11 | mdok-postman (1,909) | `postman` — v2.1 importer | moderate | 8–12 |
| 12 | mdok-cli main (6,383) | `cli` — commands, config, jobs pool, record/replay serialization | moderate/hard | 15–25 |
| 13 | mdok-cli mcp.rs (827) | `mcp` — JSON-RPC server, self-exec tools | moderate | 5–8 |
| 14 | mdok-test-server (1,180) | `testserver` — loopback fixture | trivial | 4–6 |
| 15 | mdok-command-fixture (82) | `cmdfix` | trivial | 1 |
| 16 | — | CI, SBOM/provenance, release packaging, cross-builds | moderate | 5–10 |
| 17 | — | Perf-budget calibration for the Go binary | moderate | 3–5 |
| | | **Total** | | **110–181 dev-days** |

With a 25–35% contingency for the risk items below:
**≈ 140–245 dev-days (6.5–11 person-months)** for one developer; two
developers can compress calendar time to roughly 4–6 months once the core
packages land (items 1–6 parallelize well; 9, 10, 12 are the serial spine).

## Sequencing (each stage gated by the portable suites)

| Stage | Deliver | Gate that must pass |
|---|---|---|
| 0 | Go module scaffold, CI, testserver + cmdfix ports | builds; testserver serves fixture endpoints |
| 1 | core, template, shell, markdown, jmespath + minimal `lint`/`plan`/`list` CLI | corpus plan-stage (170 cases) + golden `corpus-{lint,plan,list}` |
| 2 | report, runtime, offline `test` | corpus all stages (497) incl. report-shape validation |
| 3 | curlplan + httpx (parser, policy, execution, artifacts) | e2e 9/9, differential 655/655, TLS matrix, golden record-replay |
| 4 | quickjs cgo + `mdok-pm-probe` | postman API gate (2,305 scripts, uncovered=0) + spec gate (967/967) |
| 5 | postman importer + `import` CLI | golden import + spec-coverage gate |
| 6 | full CLI parity (record/replay, jobs, config) + MCP server | golden 8/8, MCP conformance 22/22 |
| 7 | perf budgets, SBOM/provenance, release, platform matrix | `verify_performance.py --strict`, release-smoke, TLS matrix per host |

## Risk register (ranked)

1. **QuickJS cgo binding** — no maintained Go binding exposes
   `JS_SetMemoryLimit`/`JS_SetMaxStackSize`/interrupt handlers; must be
   written and proven under the Go scheduler. *Mitigation:* week-1 spike;
   fallback to goja with an explicitly documented weakening of the sandbox
   budget contract (probe spec would need a revision).
2. **curl argv parity** — 655 differential cases, including 129
   feature-unavailable classifications that must remain identical. *Mitigation:*
   generate the Go option tables from `specs/curl-option-policy.csv`; run the
   differential from the first day of stage 3.
3. **Report byte-fidelity** — redaction covers base64/hex/percent/reversed
   encodings; golden-diff normalizes almost nothing here. *Mitigation:*
   golden 8/8 is the exit gate; port the redactor first inside stage 2.
4. **jmespath edge cases** — go-jmespath and jmespath.rs diverge on thin
   edges (error messages, coercion). *Mitigation:* 80 corpus cases + possibly
   carrying small patches on a vendored go-jmespath.
5. **Windows process semantics** — Job Objects for group kill; only
   white-box tests define the reaper contract today. *Mitigation:* extract
   those semantics into the conformance/golden harnesses during stage 3.
6. **MCP self-exec pattern** — document tools re-exec the binary with
   `os.Executable()`; child argv/env contracts (secrets via
   `MDOK_MCP_SECRET_*`) are pinned only by the new conformance suite.

## Expected non-functional deltas (measured, not guessed)

From a calibration experiment on this machine (M3 Max, Go 1.26.5, dependency
graph mirroring mdok's roles):

- Cold full build: **~57s (Rust, measured) → ~15–25s (Go, scaled estimate)**;
  warm rebuild after touching a core package: **~37s → ~1–5s**.
- Binary size: **8.6 MB → ~15–25 MB**.
- Runtime CPU: expect Rust to retain an edge in template/JMESPath/JS-heavy
  paths; the Go HTTP stack is competitive. Budgets must be re-calibrated
  (stage 7), not copied.
- Cross-compilation becomes trivial for pure-Go parts, but cgo QuickJS
  reintroduces a C toolchain dependency for every target platform.

## Explicitly out of scope for this estimate

- Keeping the Rust and Go implementations in maintenance lockstep after
  parity (choose one as primary, or fund double-testing).
- Spec-only error codes E305/E610/E701/E900 — a port decision (implement or
  drop the spec rows) is required either way and is costed at zero here.
- Any UI/web assets (`web/`), which are language-independent.
