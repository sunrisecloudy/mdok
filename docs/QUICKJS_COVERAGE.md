# MDOK QuickJS runtime and Postman coverage

Status: implemented and validated against a real-world corpus. The QuickJS
adaptation in `docs/CELLD_QUICKJS_ADAPTATION.md` is staged in; the differential
coverage harness is green (100% of the pm API surface used by the corpus is
implemented).

## Components

| Component | Path | Role |
| --- | --- | --- |
| QuickJS capability crate | `crates/mdok-quickjs` | rquickjs sandbox (stack/heap/interrupt budgets), Postman `pm` facade, named capability diagnostics, canonical transcript, coverage recording, typed child-request effect protocol |
| Probe CLI | `crates/mdok-quickjs/src/bin/mdok-pm-probe.rs` | `--case PATH|- --network offline|fetch --timeout-ms N`, `--list-api` |
| Corpus downloader | `scripts/fetch_postman_corpus.py` | downloads ~100 random Postman collections with JS scripts into `tests/corpus/postman-js/` |
| Coverage runner | `scripts/run_postman_coverage.py` | runs every script through the probe, aggregates used API surface, enforces the gate |
| Probe/coverage contract | `docs/QUICKJS_PROBE_SPEC.md` | JSON schemas for probe cases/outputs, pm surface, gate semantics |
| Vendored spec | `vendor/postman-collection-spec/` | official Postman Collection JSON Schema v2.1.0/v2.0.0 + draft-07 + upstream provenance + README |
| Spec coverage checker | `scripts/check_postman_spec_coverage.py` | walks the vendored schema, checks mdok-postman importer + mdok-quickjs coverage (gate: missing = 0) |
| Spec coverage report | `docs/POSTMAN_SPEC_COVERAGE.md` | element-by-element coverage table (967 elements: 408 supported, 399 diagnosed, 120 informational, 40 containers) |

## Coverage contract (100% rule)

Every API/element is EITHER implemented OR fails with a named compatibility
diagnostic (MDOK-PM-*). Nothing is silently dropped or returns an empty
stub. `mdok-pm-probe --list-api` enumerates the supported surface; the
coverage runner fails unless `used - supported` is empty.

## Corpus gate (latest run)

```
python3 scripts/run_postman_coverage.py --probe target/release/mdok-pm-probe
# exit 0; report: target/postman-coverage/report.{json,md}
```

- 100 collections (58 source repos), 2305 scripts (1468 test + 272 prerequest
  events; the runner also counts inherited folder events)
- 38 distinct pm APIs used, 4909 total uses — all supported (183 paths +
  5 modules)
- Outcomes: 1086 passed / 528 failed / 690 error / 1 timeout. The error
  bucket is scripts using out-of-profile features (undefined legacy globals,
  collection-runner APIs, eval, offline sendRequest) failing loudly per the
  contract, plus genuinely broken scripts.

## Notable implemented surfaces (beyond the spec's base list)

- Legacy Postman sandbox globals: `responseBody`, `responseCode`,
  `responseHeaders`, `responseTime`, `tests[...]` assignments, `environment`,
  `globals`, `data`, `iteration`, `postman.*` (getResponseHeader,
  setEnvironmentVariable, setNextRequest, ...), `_` (lodash), `xml2Json`.
- Timers: `setTimeout`/`setInterval`/`clearTimeout`/`clearInterval`, pumped by
  the Rust shell between promise-job drains and bounded by the script
  deadline (a wait-style `setTimeout(fn, 6000)` faithfully times out at the
  2s budget, exactly like the Postman runner).
- `xml2Json`: xml2js semantics with the official postman-sandbox option set
  (`{explicitArray:false, trim:true, mergeAttrs:false}`) — namespaced keys,
  attributes under `$`, text under `_`, repeated elements as arrays, CDATA and
  entities; malformed XML returns `{}`.
- `pm.response.to.not.*` negation, `to.have.{ok,success,redirection,
  clientError,serverError,error}`, `to.be.{info,json,withBody}`, `pm.payload`
  alias of the response.
- Modules: vendored `lodash` 4.17.21, `moment` 2.29.4, `crypto-js` 4.2.0 UMD
  bundles, `ajv` 6.12.6 browser bundle (compile is hardened: `new Function` ->
  MDOK-PM-EVAL), `uuid`/`querystring` capability shims; internal-only shims
  (e.g. `crypto`) satisfy module-bundle probes without being advertised or
  recorded.
- Secrets/taint: secret values never appear in transcripts, logs,
  diagnostics, child-request records, or exception text.

## Reproduce

```sh
cargo test -p mdok-quickjs          # 33 tests, all under a 30s watchdog
cargo test -p mdok-postman          # importer tests
cargo clippy -p mdok-quickjs -p mdok-postman --all-targets -- -D warnings
cargo build --release -p mdok-quickjs --bin mdok-pm-probe
python3 scripts/run_postman_coverage.py --probe target/release/mdok-pm-probe   # corpus gate
python3 scripts/check_postman_spec_coverage.py                                 # spec gate
```

Current status (2026-08-06): **both gates pass** — corpus gate uncovered=0
(100 collections / 2305 scripts / 39 distinct APIs, 184 supported paths,
modules incl. lodash/moment/ajv/uuid/querystring/crypto-js), spec gate
missing=0 (967 schema elements). Corpus outcomes: 1097 passed / 531 failed /
612 error / 65 timeout; the error remainder is response-data mismatches from
the offline canned fixtures (scripts expecting live API payload shapes) plus
documented out-of-profile failures (eval 5, offline sendRequest 54) — no
engine crashes, timers/xml2Json/`_`/crypto-js buckets resolved.
