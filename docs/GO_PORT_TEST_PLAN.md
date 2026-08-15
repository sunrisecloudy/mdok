# Go Port Test Plan

Status of the test assets for a full Go port of mdok (target architecture:
**hybrid** — pure-Go HTTP execution via `net/http`, QuickJS retained via cgo),
as audited on branch `go-port/test-audit`.

## Verdict

**The black-box suite is sufficient to gate a port.** mdok's heaviest
validation is language-agnostic and drives the CLI/MCP binary rather than
Rust internals: a 497-case corpus, a 655-case curl differential, a 2,305-script
Postman corpus with two coverage gates, 9 E2E loopback workflows, and JSON
schemas for every output format. The audit added the two missing pieces — an
MCP wire-protocol conformance suite and a golden-diff harness — so every
externally visible surface is now pinned by a portable test.

What does **not** port: the 204 white-box Rust tests (unit + integration).
They remain the semantic reference for a Go re-implementation but must be
re-authored, not translated.

## Measured black-box coverage

Line coverage of workspace crates from portable suites only (no `cargo
test`), measured with llvm-cov on instrumented `target/llvm-cov/debug/mdok`
and `mdok-pm-probe`, rust-toolchain 1.96.0, 2026-08-15:

| Crate | corpus (495) | e2e (9) | differential (655) | postman (2305) | MCP conformance | Combined |
|---|---|---|---|---|---|---|
| mdok-cli | 37.0% | 28.8% | 20.8% | 0% | see below | **49.0%** |
| mdok-command | 0% | 0% | 0% | 0% | — | **48.6%** |
| mdok-core | 51.9% | 46.5% | 14.1% | 0% | — | **51.9%** |
| mdok-curl | 68.0% | 45.9% | 21.0% | 0% | — | **69.7%** |
| mdok-curl-sys | 68.5% | 61.8% | 0% | 0% | — | **68.5%** |
| mdok-markdown | 72.0% | 64.3% | 49.1% | 0% | — | **80.0%** |
| mdok-postman | 0% | 0% | 0% | 0% | — | **31.3%** |
| mdok-quickjs | 0% | 0% | 0% | 72.0% | — | **73.3%** |
| mdok-report | 62.0% | 39.8% | 41.6% | 0% | — | **66.4%** |
| mdok-template | 59.1% | 49.0% | 10.7% | 0% | — | **59.1%** |
| **Total** | | | | | | **56.7%** |

Notes:

- The MCP conformance column is folded into "combined" (it was measured as a
  fifth stage; it alone lifted the total from 47.0% to 56.7%, mdok-command
  from 0% to 48.6%, and mdok-postman from 0% to 31.3%).
- Remaining low areas are deliberate targets for the golden-diff harness
  (record/replay, import CLI paths) rather than corpus cases.
- mdok-curl-sys coverage measures the Rust FFI wrapper only; the C bridge
  itself is validated by sanitizers and the differential, and disappears in
  the hybrid port's HTTP path.

## Error-code audit

`specs/error-codes.md` declares 34 `MDOK-E###` codes.

- **28 exercised by portable suites** after this audit added corpus cases for
  `MDOK-E200` (trailing escape, T0496) and `MDOK-E404` (template depth limit,
  T0497). `MDOK-E001` (invalid UTF-8) and `MDOK-E800` (report write failure)
  are exercised by the golden-diff harness (binary fixtures and unwritable
  report paths cannot live in the corpus: the manifest validator requires
  UTF-8-readable Markdown with header markers).
- **4 codes are spec-only and not implemented anywhere**: `MDOK-E305`
  (build feature unavailable), `MDOK-E610` (JSON body parse failure),
  `MDOK-E701` (execution cancelled), `MDOK-E900` (internal invariant/FFI
  error). A port must decide per code whether to implement or drop the spec
  row; they cannot be tested today because no code path emits them.

## Asset classification

### Reused as-is by a Go port (no rewrite)

| Asset | Location | Gate for |
|---|---|---|
| Corpus, 497 cases + manifest validator | `tests/corpus/`, `mdok-prd/tests/corpus/` | lint/plan/test/report behavior, error codes |
| Corpus runner | `scripts/run_corpus.py --binary <go-mdok>` | same, against any binary |
| Markdown E2E runner + workflows | `scripts/run_md_e2e.py --binary ...`, `tests/e2e/` | loopback execution incl. TLS |
| curl differential (655 cases) | `scripts/run_curl_differential.py --mdok ...` + vendored curl 8.21 | argv-parse accept/reject parity vs real curl |
| Postman corpus (100 collections, 2,305 scripts) | `tests/corpus/postman-js/` | JS runtime behavior |
| Postman coverage gates | `scripts/run_postman_coverage.py --probe <go-probe>`, `scripts/check_postman_spec_coverage.py` | pm.* API coverage; schema-element coverage |
| MCP conformance suite (new) | `scripts/run_mcp_conformance.py --server <go-mdok>`, `tests/mcp/` | wire protocol, tool inventory, F5/F7/F9 |
| Golden-diff harness (new) | `scripts/run_golden_diff.py --binary <go-mdok>`, `tests/golden/` | byte-level output parity (see below) |
| Output JSON schemas | `specs/{report,call,response,corpus-manifest,release-manifest}.schema.json` | report/plan/call shapes |
| curl option policy | `specs/curl-option-policy.csv` (289 rows) | option classification parity |
| Fuzz seed corpora | `fuzz/corpus/` (8.3k markdown, 905 shell/template, 33 curl argv) | Go fuzzing seeds |
| Perf budgets | `scripts/bench_performance.py` + `verify_performance.py --strict` | process-level ms/RSS targets |
| TLS matrix | `scripts/run_tls_matrix.py` | per-host TLS/session gates |
| Fixture server certs, vendored JS prelude (374 KB), Postman spec | `crates/mdok-test-server/`, `crates/mdok-quickjs/src/`, `vendor/postman-collection-spec/` | reusable inputs |

### Re-authored in Go (semantic reference only)

- 204 Rust test functions (126 unit + 78 integration): unit semantics of
  mdok-core/template/shell/markdown/jmespath/runtime/report/curl/postman/
  quickjs, plus `crates/mdok-cli/tests/commands.rs` (CLI behaviors incl.
  record/replay internals and exec policy).
- Criterion allocation-budget benchmarks (`CountingAllocator` ceilings do not
  exist in Go; replace with `testing.B` + pprof-based budgets).
- Sanitizer suite (`scripts/run_sanitizers.sh`, curl-FFI only). The hybrid
  port keeps cgo for QuickJS and needs an equivalent safety harness there;
  the curl bridge sanitizer tests retire with the bridge itself.

## New suites added by this audit

1. **MCP conformance** — `make mcp-conformance`. 22 cases over stdio JSON-RPC:
   initialize handshake, `-32601` on unknown methods, exact 7-tool inventory
   with schemas, document tools re-exec + result wrapping, offline probe
   validation (network enum, timeout bounds), F7 budget clamping, import
   input validation, F9 read-root confinement, F5 config-ignoring, secrets
   never in responses. Binary-agnostic; runs against any server binary.
2. **Golden-diff** — `make golden` (re-capture: `make golden-update`). Pins
   normalized stdout/exit for: `version --json`; lint/plan/list across all
   170 plan-stage corpus docs; Postman import; `MDOK-E001` and `MDOK-E800`
   edge cases; and a record → `replay --strict` round trip against the
   fixture server. Normalizes durations, run ids, timestamps, ports, temp
   paths, version strings, OS error text, and port-dependent hashes. This is
   the primary Rust-vs-Go acceptance gate.

## Known remaining gaps

- Coverage of `mdok-postman` (31%) and `mdok-cli/main.rs` (~42%) from
  black-box suites alone; the white-box tests cover the rest but do not port.
  The Postman spec-coverage gate compensates for importer element coverage.
- No automated MCP tests exercise a real MCP *client* SDK handshake variance
  (only the canonical handshake); acceptable for a controlled port.
- Branch coverage was not measurable (rustc 1.96 instrumentation emits no
  branch counters with the flags used); line coverage was used throughout.
- Spec-only error codes E305/E610/E701/E900 (see above).

## Hybrid-port-specific gates

- **curl argv parity**: the differential (655 cases vs pinned curl 8.21)
  becomes the gate for the pure-Go argv parser; the C parser bridge retires.
- **JS runtime parity**: the Postman corpus + both coverage gates validate
  the cgo QuickJS port; `mdok-pm-probe --list-api` output must match.
- **HTTP behavior**: corpus execute-stage + e2e + TLS matrix gate the
  net/http execution engine (redirects, cookies, auth, gzip, TLS policy).
- **Output fidelity**: golden-diff must pass byte-for-byte after
  normalization for the Go binary.
