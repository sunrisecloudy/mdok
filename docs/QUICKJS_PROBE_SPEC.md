# MDOK QuickJS probe & coverage contract

Implements the QuickJS adaptation in `docs/CELLD_QUICKJS_ADAPTATION.md` and the
differential-corpus coverage gate. Three workstreams share this contract:

1. Rust crate `crates/mdok-quickjs` + probe binary `mdok-pm-probe`.
2. Python corpus downloader `scripts/fetch_postman_corpus.py` (~100 random
   Postman collections with JS scripts).
3. Python coverage runner `scripts/run_postman_coverage.py`.

## 1. Crate: crates/mdok-quickjs

New workspace member (`crates/mdok-quickjs/Cargo.toml`, add to workspace
`members` in the root `Cargo.toml`). Edition 2024, rust-version 1.85,
`#![forbid(unsafe_code)]`. Dependencies from workspace where available:
`serde`, `serde_json`, `thiserror`, `sha2`; add `rquickjs = { version = "0.12",
features = ["macro"] }` (matches the Terrane boundary at
`/Users/vehasuwat/Project/terrane/rust/crates/terrane-cap-js-runtime`; read
`src/sandbox.rs` there as the reference shape: Runtime::new, max stack 512KB,
memory limit, interrupt handler with injected deadline, Context::full, eval,
catch + first-error capture).

Files:
- `src/lib.rs` — public API, profile/input/output types, `run_script()`.
- `src/sandbox.rs` — runtime setup, budgets, deadlines, eval, first error.
- `src/pm.rs` — the Postman `pm` facade (the big one) + coverage recording.
- `src/transcript.rs` — canonical serializable transcript.
- `src/effect.rs` — typed child-request effect protocol (op ids, generation).
- `src/modules.rs` — pinned `require()` registry.
- `src/bin/mdok-pm-probe.rs` — CLI probe (see section 3).
- `tests/` — integration tests with realistic Postman scripts.
- `tests/fixtures/` — canned JS modules if vendored (see modules).

## 2. Probe CLI

```
mdok-pm-probe --case PATH.json [--network offline|fetch] [--timeout-ms N]
mdok-pm-probe --case -            # read case from stdin
mdok-pm-probe --list-api          # emit supported API surface JSON, exit 0
```

`--network` default: `offline`. Errors: print `{"ok":false,"error":"..."}` to
stdout, exit 1. Normal run: print output JSON to stdout, exit 0.

### Case input JSON (probe stdin/file)

```json
{
  "script": "pm.test("status", function(){ pm.response.to.have.status(200); });",
  "phase": "test",
  "request": {
    "name": "Get user",
    "method": "GET",
    "url": "https://api.example.test/users/1",
    "headers": [{"key":"X-Token","value":"opaque-secret"}],
    "body": null
  },
  "response": {
    "code": 200,
    "status": "OK",
    "headers": [{"key":"Content-Type","value":"application/json"}],
    "body": "{\"id\":1,\"name\":\"ada\"}",
    "response_time_ms": 12,
    "response_size_bytes": 42
  },
  "variables": {
    "global": {"api_url": "https://api.example.test"},
    "collection": {"user_id": "1"},
    "environment": {"token": "opaque-secret"},
    "data": {"row": "x"},
    "local": {}
  },
  "secrets": ["token", "X-Token"],
  "profile": {
    "api_version": "postman-cli-v1",
    "script_timeout_ms": 2000
  },
  "coverage": true
}
```

Fields optional where obvious (`response` may be absent for prerequest phase).
`secrets` lists variable/header names whose values are tainted: they may be
read by the script, but must never appear in the transcript, logs,
diagnostics, child-request records, or exception text (mask as `[redacted]`).

### Output JSON

```json
{
  "ok": true,
  "outcome": "passed|failed|error|timeout",
  "duration_ms": 3,
  "used_api": ["pm.test", "pm.response.to.have.status", "pm.environment.set"],
  "diagnostics": [
    {"code": "MDOK-PM-UNSUPPORTED", "api": "pm.vault.get", "message": "..."}
  ],
  "transcript": {
    "tests": [{"name": "status", "passed": true, "error": null}],
    "scope_writes": [{"scope": "environment", "key": "user_id", "value": "1", "redacted": false}],
    "logs": [{"level": "log", "message": "hello"}],
    "errors": [],
    "child_requests": [{"op": 1, "method": "GET", "url": "https://...", "status": 200, "error": null, "resolved": true, "redacted": false}],
    "control_flow": [{"action": "skip_request", "value": null, "supported": true}],
    "visualizer": null
  }
}
```

`outcome`: `passed` (no failed test and no error), `failed` (≥1 failed test,
script ran to completion), `error` (exception escaped / syntax error), `timeout`
(interrupt fired). `ok` is false only on harness-level errors (bad input,
runtime setup failure) — script errors are `ok:true, outcome:"error"`.

### --list-api output

```json
{
  "profile": "postman-cli-v1",
  "supported": ["pm.test", "pm.expect", "pm.info.eventName", "...", "require:lodash"],
  "modules": ["lodash"],
  "diagnostic_codes": ["MDOK-PM-UNSUPPORTED", "MDOK-PM-NETWORK-OFFLINE", "MDOK-PM-REQUIRE", "MDOK-PM-SECRET-DENIED", "MDOK-PM-TIMEOUT", "MDOK-PM-LIMIT", "MDOK-PM-EVAL"]
}
```

`supported` enumerates every recorded API path the facade exposes (see the
recording rule). `require:<module>` entries represent module availability.

## 3. pm facade surface (profile `postman-cli-v1`)

All of the following must exist and behave per the official Postman sandbox
contract. Anything else accessed on `pm` (or on a known pm object) must record
the path and emit `MDOK-PM-UNSUPPORTED` once per distinct path, then throw on
use (never silently return an empty object).

- `pm.test(name, fn)` — run fn, record `{name, passed, error}` in transcript;
  exception inside fn = failed test with error text (redacted), not a thrown
  script error.
- `pm.expect(value)` — chai-compatible assertion object supporting (chainable,
  with `.not` and `.and`): `equal/eql` (+ `.deep.equal`), `include/contain`,
  `match(regex)`, `satisfy(fn)`, `property(name[,value])`, `nested.property`,
  `lengthOf(n)`, `keys(...)`, `oneOf([...])`, `above/below/at.least/at.most/
  within/lessThan/greaterThan`, `instanceOf(K)`, `a(type)/an(type)`
  ("string","number","boolean","object","array","function","undefined","null",
  "date","regexp"), `true/false/null/undefined/ok/empty/nan`, `throw()`,
  `status(n)`, `header(name)`, `jsonBody()`, `jsonSchema(schema[,opts])`
  (accept an object; full JSON-schema validation is best-effort — validate a
  small subset: required/type/properties; otherwise pass). Assertion failure
  throws an AssertionError whose message goes into the test error. Also expose
  the chai-style `to.be.a`/`an` etc. chains.
- `pm.info` — `eventName` (phase string), `iteration` (0), `iterationCount`
  (1), `requestName`, `requestId`.
- `pm.request` — `method`, `url` (string), `headers` (object: `get(name)`,
  `has(name)`, `toObject()`, `count()`, plus indexed access), `body`
  (`mode`, `raw` or `null`, `toJSON()`), `auth` (object or null), `data`
  (alias for body).
- `pm.response` — `code`, `status`, `responseTime`, `responseSize`, `headers`
  (same header object shape), `text()`, `json()` (throws on invalid JSON),
  `toJSON()`, `responseCode` (alias of code); chain objects
  `pm.response.to.have.status(n|regex)`, `.header(name)`, `.body(str)`,
  `.jsonBody()`, `.jsonSchema(schema)`, and `pm.response.to.be.ok/success/
  redirection/clientError/serverError/error`.
- Variable scopes — `pm.variables`, `pm.environment`, `pm.globals`,
  `pm.collectionVariables`, `pm.iterationData`: each has `get(name)`, `set(name,
  value)`, `has(name)`, `unset(name)`, `replaceIn(template)`, `toObject()`.
  Precedence for `pm.variables.get`: global → collection → environment → data →
  local. `pm.variables.set` writes local. Writes record `scope_writes` in the
  transcript (redacted when the key is a secret or value is tainted). `set`
  with a non-scalar value stringifies like Postman (JSON for objects/arrays).
- `pm.cookies` — `get(name)`, `has(name)`, `toObject()` (seeded from
  `response.headers` Set-Cookie when present, else empty).
- `pm.sendRequest(urlOrOptions, callback?)` — returns a Promise. Accepts a URL
  string or an options object `{url, method, header:{...}|[{key,value}],
  body:{mode,raw}|string, auth?}`. Emits a child-request effect with a fresh op
  id. In `--network fetch` mode the probe shell performs the request with
  reqwest (timeout from profile, follow redirects, 8MB body cap, redact secret
  headers/body from the transcript); in `offline` mode the promise rejects with
  `MDOK-PM-NETWORK-OFFLINE` and the child_request record has `error` set,
  `resolved:false`. Callback form `(err, res)` and Promise/await form must
  produce the same transcript. Response object exposed to JS: `code`, `status`,
  `headers`, `text()`, `json()`, `responseTime`, `responseSize`. Nested
  `pm.sendRequest` from a callback is allowed (op ids keep increasing). If a
  script never settles its promises before the interrupt deadline, outcome =
  `timeout`.
- `pm.execution` — `setNextRequest(name)` (records control_flow, supported),
  `skipRequest()` (records control_flow, supported), `runRequest(name, cb)`
  (records control_flow with `supported:false` and emits
  `MDOK-PM-UNSUPPORTED` diagnostic; collection-runner-only API).
- `pm.visualizer.set(template, data)` — bounded (template ≤ 64KB, data ≤ 1MB
  serialized); records `visualizer` in transcript.
- `pm.vault.get(name)` — secret-gated. If `name` is in case `secrets` (or the
  value was supplied), resolve the value (Promise) and mark it redacted in
  logs; otherwise reject with `MDOK-PM-SECRET-DENIED`. Reads are never written
  to the transcript.
- `console.log/info/warn/error/debug(...)` — bounded (100 entries, 4KB each),
  recorded in `logs`, values redacted when they contain tainted data.
- `require(name)` — pinned registry `src/modules.rs`. Vendor `lodash` 4.17.21
  as a pure-JS bundle (fetch
  `https://cdn.jsdelivr.net/npm/lodash@4.17.21/lodash.min.js` once during dev
  and check it in at `crates/mdok-quickjs/src/modules/lodash.js`; if the fetch
  fails, ship a module that emits `MDOK-PM-REQUIRE`). Eval it inside the
  QuickJS context with a `module/exports` shim. `require` of anything else →
  `MDOK-PM-REQUIRE` diagnostic + throw. `require` is recorded as
  `require:<name>` in used_api.
- Hardened profile (`eval`/`Function`): set `globalThis.eval` and
  `globalThis.Function` to undefined (Terrane does this). If a script calls
  them, that is a script error; record `MDOK-PM-EVAL` diagnostic.

## 4. Coverage recording rule

When `coverage:true` (default), every *leaf* property access on the pm object
tree is recorded in `used_api`. A get is a leaf when it returns a non-object
value or a function; container objects (like `pm.response.to`) are traversed
but not recorded. `require:<name>` is recorded when called. Unknown members
record the attempted path too (they also emit MDOK-PM-UNSUPPORTED). Dedupe,
preserve first-use order. Implement with a JS prelude (like Terrane's
`runtime/app_runtime.js`) that wraps the installed `pm` in a recording Proxy,
or with Rust-installed getters that call a recorder — your choice, but the
recorder must see every leaf access.

## 5. Budgets & limits (sandbox.rs)

- max stack 512KB (rquickjs `set_max_stack_size`), memory limit 64MB
  (`set_memory_limit`), configurable via case `profile`.
- Interrupt handler checks an injected deadline (`script_timeout_ms`, default
  2000): return true → stop, outcome `timeout`, diagnostic `MDOK-PM-TIMEOUT`.
- Logs: 100 entries × 4KB. Transcript strings truncated at 64KB.
- After the script settles (sync completion + all promise jobs drained via
  `ctx.run_jobs()` until quiescent or deadline), fold results.

## 6. Determinism & secrets

- No ambient filesystem/process/sockets/wall-clock/randomness host APIs beyond
  QuickJS's built-in Date/Math (compat profile allows them; the runtime core
  will inject clocks later — not this crate's job).
- Tainted values (from `secrets` names, secret-looking keys via the same
  heuristic as `crates/mdok-postman/src/lib.rs` `looks_secret`) must never
  appear in: transcript, logs, diagnostics, child_requests, exception text,
  test error text, scope_writes values. Mask `[redacted]`.

## 7. Tests (integration, `cargo test -p mdok-quickjs`)

Write realistic Postman test-script fixtures covering at least:
- `pm.test` pass/fail + `pm.response.to.have.status`, `.jsonBody()`,
  `.to.be.ok`, `pm.expect(...).to.eql/include/oneOf/be.a/have.property`.
- variable precedence + `pm.environment.set` + `pm.variables.get/replaceIn`.
- `pm.sendRequest` offline rejection and fetch success (fetch mode against the
  local `mdok-test-server` or a loopback httptest-style listener; simplest:
  spin up a tiny std TcpListener in the test serving one canned response).
- coverage recording: assert exact `used_api` for a fixture script.
- secrets: assert no tainted value appears anywhere in transcript JSON.
- timeout: a busy-loop script must yield outcome `timeout` within budget.
- `pm.expect` failure → failed test with error, not a script crash.
- `require('lodash')` resolves and `_.get` works; unknown require → diagnostic.
- unknown `pm.foo` → MDOK-PM-UNSUPPORTED and throws on use.

## 8. Python corpus downloader — scripts/fetch_postman_corpus.py

Stdlib + requests (pre-installed). No API tokens. Goal: download ~100 random
Postman collections that contain ≥1 JS script (test or prerequest `event`),
saved under `tests/corpus/postman-js/`:

- `collections/<index>-<sanitized-name>.json` — raw collection JSON (v2.1).
- `corpus.json` — manifest: `{version, seed, fetched_at, entries: [{index,
  name, source_url, sha256, byte_size, script_count, script_events: {test: n,
  prerequest: n}, js_scripts: [sha256...]}]}`.

Strategy (must work unauthenticated):
1. GitHub repo search API (`https://api.github.com/search/repositories`,
   10 req/min unauthenticated; 100 results/page). Queries:
   `topic:postman`, `postman collection in:name`, `postman-collection in:name`,
   `postman api in:name,description`, `newman in:name`. Collect candidate
   repos, dedupe, shuffle with `--seed` (default 0).
2. For each repo (parallel, ≤8 workers): `git clone --depth 1
   --filter=blob:none --sparse` into a temp dir (under `target/postman-corpus-
   clones/`), `git ls-tree -r --name-only HEAD`, find collection-like paths:
   `*.postman_collection.json`, `*.postman_collection`, `postman/*.json`, and
   any `*.json` whose content has `info.schema` containing
   `collection/v2.1` and an `item` array; for candidates, `git show
   HEAD:<path>` (partial-clone fetches the blob on demand). Keep collections
   with ≥1 `event` whose `script.exec` is a non-empty string/array containing
   `pm.`. Cap: skip files > 8MB; stop when `--limit` (default 100) collected.
3. Robustness: retry transient failures once; skip repos that fail clone or
   have no candidates; log progress to stderr (collection count so far); honor
   GitHub search rate limit by sleeping until reset (parse the `X-RateLimit-
   Reset` header) — the corpus should reach the limit even on a slow network.
4. `--resume` skips already-downloaded entries; `--force` re-downloads.
5. Print a summary: counts, source repo histogram, script-event histogram.

Reproducibility: seed + recorded source URLs. Randomness = shuffled repo order,
not content selection — the corpus is whatever the sampled repos contain.

## 9. Python coverage runner — scripts/run_postman_coverage.py

Reads `tests/corpus/postman-js/corpus.json` + collection files. For each
collection (parallel, ≤8 workers):

1. Parse JSON; walk `item` recursively (folders/requests), collecting every
   `event` (listen `test`|`prerequest`) with script source (join `exec` array).
2. Build a probe case: `phase` from listen; `request` from the owning item
   (method, url raw string, headers, body raw); `response` from the item's
   first saved example `response[0]` if present (code, status, headers, body),
   else canned `{"code":200,"status":"OK","headers":[...],"body":"{\"ok\":true}"}`;
   `variables` from collection `variable` arrays (seed `collection` scope;
   put names matching the postman `looks_secret` heuristic into `secrets`);
   `profile` with `script_timeout_ms: 2000`.
3. Run `mdok-pm-probe --case -` (stdin) with `--network offline`; parse output.
4. Collect: used_api (union + per-path counts), diagnostics (dedupe by
   code+api), outcomes histogram, per-collection transcript stats.

Aggregation and gate:
- `used` = union of used_api. `supported` = `--list-api` output.
- `uncovered` = `used - supported`. Gate: `uncovered` must be empty. Anything
  in `uncovered` must also have produced `MDOK-PM-UNSUPPORTED` (it did, by
  construction) — but for the gate, supported must cover it, so implement the
  API rather than relying on the diagnostic.
- Report `target/postman-coverage/report.json` (machine) and
  `target/postman-coverage/report.md` (human): corpus size, collections with
  scripts, scripts run, outcomes histogram, top used APIs with counts, full
  used list, uncovered list (must be empty for 100%), diagnostics summary.
- Exit 0 iff corpus non-empty AND uncovered empty; else exit 1.
- `--probe PATH` (default `target/release/mdok-pm-probe`, fall back to
  `cargo run -p mdok-quickjs --bin mdok-pm-probe --`), `--corpus
  tests/corpus/postman-js`, `--out target/postman-coverage`, `--limit N`
  (cap collections), `--workers 8`.

## 10. Definition of done ("100% coverage")

The full pipeline passes: corpus downloader fetches ≥100 collections with JS
scripts; probe crate compiles with tests green; coverage runner reports
`uncovered` empty (every pm API used by the corpus is implemented, and
everything else fails with a named diagnostic per the profile contract).
