# Rust → Go Port Playbook

Method, test strategy, planning, and pitfalls from porting mdok (Rust, 16
crates, ~25k LOC + vendored C curl + QuickJS) to Go — validated by an 891-case
differential parity suite at 100% agreement, then shipped through Homebrew.
Written to be reused on another Rust project.

---

## 1. The method that worked

**Do not start by porting code. Start by proving you can detect divergence.**

The order that worked:

1. **Audit the test estate first.** Classify every test asset as *portable*
   (drives the binary black-box: corpus runners, e2e scripts, schemas, fuzz
   seeds) or *white-box* (unit tests bound to Rust internals — they do not
   port, they only serve as a semantic reference). Measure line coverage of
   the **portable** suites only, using instrumented builds
   (`cargo-llvm-cov`, run the external runners against the instrumented
   binary). Gaps you find are gaps the port will inherit.
2. **Cross-reference the behavioral spec against reality.** For mdok: 34
   documented error codes → 26 exercised by portable tests, 4 documented but
   *implemented nowhere* (spec ghosts — a port decision, not a port bug), 4
   real but untested. Fix the gaps **in the Rust repo first**, while it is
   still the reference.
3. **Make every runner binary-parameterized** (`--binary`, `--server`). This
   one flag turns your existing CI scripts into port gates for free.
4. **Freeze shared contracts before writing implementations.** One types
   file (document, plan, transfer, diagnostic, report) that both languages'
   stages compile against. Every agent/porter builds against it. Contract
   drift discovered late cost us an hour; the frozen contract prevented it
   three more times.
5. **Port in dependency order, gated at each stage** (see §2).
6. **Build a differential parity harness and iterate to zero.** This — not
   the unit tests — is what actually certifies the port (see §3).
7. **Ship, then keep the parity suite as a permanent gate.**

## 2. The plan

| Stage | Deliver | Gate that must pass |
|---|---|---|
| 0 | Scaffold target module, fixture/test-server port | builds; fixture verified against the original **byte-for-byte** (mdok: 93/93 endpoint diff) |
| 1 | Leaf logic packages (parsers, template, policy) + minimal CLI | existing e2e suite against the new binary |
| 2 | Orchestration (runtime, reports) | full corpus + e2e |
| 3 | The hard core (protocol/FFI/HTTP engine) | differential parity suite at 0 mismatches |
| 4 | Remaining surface (import, MCP, …) | their dedicated conformance suites |
| 5 | Packaging (cross-compile, release, tap) | install the artifact and re-run e2e **through the installed binary** |

Effort calibration from this port: the estimate (docs/GO_PORT_ESTIMATE.md)
said 110–181 dev-days for full parity; the first milestone (e2e 9/9 + 891/891
parity on the ported surface) took roughly 2 days with heavy subagent
parallelism — because the test audit had already been done. Budget ≈ 40%
tests/harness, 60% implementation.

### Subagent parallelization (if you use agents)

- **Coordinator owns**: contracts file, integration layer (runtime + CLI),
  the parity harness, and all cross-cutting decisions.
- **Each agent owns one non-overlapping directory** with a spec that names
  the Rust sources to port and the exact function signatures to expose.
- **Forbid dependency mutation** (`go get`/`go mod tidy`) in agent prompts —
  concurrent module edits race. Pre-install all dependencies yourself.
- Expect **API drift** from agents (one defined its own `Transfer` type when
  the shared one was late). Reconcile at integration; it cost minutes, not
  hours, because signatures were specced.
- Have agents **verify against the reference implementation directly** (the
  fixture-server agent diffed its Go server against the running Rust server,
  93/93) — that agent's deliverable needed zero coordinator fixes.

## 3. Test case strategy

Four layers, in the order they catch bugs:

1. **E2E suite** (9 workflows): the smoke gate. Cheap, human-readable, runs
   everywhere. Necessary, wildly insufficient — the Go port passed 9/9 while
   still diverging on 100% of the deeper matrix.
2. **Differential parity suite** (~900 generated cases, the centerpiece):
   - **Deterministic generation**: fixed matrices (methods × bodies ×
     encodings, redirect hops × limits, retry counts, status codes) plus a
     seeded RNG composer for randomized-but-valid multi-step workflows. Same
     seed ⇒ same corpus ⇒ reproducible failures.
   - **Both binaries, one shared fixture**, per case: run `rust ... lint/test`
     and `go ... lint/test` against the same server, compare.
   - **Compare a projected common view**, not raw output: exit code,
     document status, per-step (name, status), and the **multiset of error
     codes**. Not messages (wording legitimately differs); not raw JSON
     (each impl emits extra fields the other lacks — raw comparison produced
     100% false mismatches on the first run).
   - **Normalize volatility** before comparing: durations, run ids,
     timestamps, ports, temp paths, version strings, and hashes over
     port-bearing content.
   - **Stateful fixtures need per-side isolation**: anything stateful (retry
     counters, CRUD stores) must be keyed by binary side, e.g. inject a
     `--var mdok_side=rust|go` and derive the fixture key from it. Without
     this, the first binary consumes the state and the second sees different
     behavior — a false mismatch that looks exactly like a real one.
3. **Harness self-check**: run the suite *reference-vs-reference* before
   trusting any result. This catches nondeterminism and normalization bugs.
   Ours passed 60/60 immediately — after we had already been bitten by a
   stale readiness file making a real run lie (175/495 with a dead server).
4. **Behavioral probes on the reference**: whenever parity disagrees, do not
   reason it out — **run the Rust binary on a minimal reproduction** and
   read its actual output (exit code, status, steps, diagnostics). Every
   hard-won finding in §5 came from a probe, not from reading source. One
   HEAD-request quirk took five probes to crack.

## 4. What to measure before porting (and what it told us)

- **Portable-suite line coverage** per crate (56.7% combined here) → where
  the port would fly blind (mdok-command and mdok-postman were at 0%).
- **Error-code / error-class inventory**: spec vs tests vs implementation.
  Found 4 unimplementable codes (spec ghosts) and 4 untested ones.
- **Output schema stability**: JSON schemas + golden files exist? If not,
  write the golden harness *before* the port (see §5's report-fidelity
  pitfall).
- **Build/cache/binary baselines** (for the decision memo): cold 46.5s →
  3.7s, incremental ~37s → 0.4s, cache 1.4 GB → 110 MB, binary 8.6 → 8.8 MB.

## 5. Pitfalls (each one cost real debugging time)

### Behavioral divergences — the ones a type system cannot see

1. **Exit-code taxonomy.** Rust used 4 classes (0 pass / 1 assertion / 2
   plan-static / 3 transfer+policy) where the port had 2. Users and CI
   scripts branch on exit codes; they are API.
2. **Silent vs reported failures.** Failed checks in mdok mark the *step*
   failed with **no document-level diagnostic**; the detail lives inside
   step sub-objects. If your reference nests failure detail, a flat port
   will "fail correctly" with the wrong shape.
3. **Plan-time vs runtime validation split.** Template render errors and
   expression compile errors failed at *plan* time (exit 2, `steps: []`,
   nothing executed) — but lint validated template *structure* without
   variable *existence* (CLI vars don't exist at lint). Port each phase's
   exact validation set.
4. **Diagnostic cascades.** A host denial emitted a *pair* of codes
   (planner E304 + policy E302); scheme denials doubled one code; an invalid
   CA path changed the pair; template errors cascaded only when the URL
   operand was a literal denied host, not a template. These orderings are
   un-guessable — encode them empirically from probes, case by case.
5. **Option arity is semantics.** `curl --form URL` *consumes* the URL
   operand, so the failure was "exactly one URL required", not "unknown
   option". Unknown-option handling must still consume arguments per the
   reference's arity table. Also: `-sS` bundling was rejected by the Rust
   parser itself — read what the reference rejects before accepting it.
6. **Tiny shell-taxonomy differences.** Unterminated quote vs trailing
   backslash vs empty fence vs leading assignment mapped to three different
   error codes. Adjacent codes, inverted expectations — pure probe material.
7. **HTTP client defaults differ.**
   - Go's **transparent gzip** (auto `Accept-Encoding`) changed *server*
     behavior vs curl. Fix: `DisableCompression: true`, add the header and
     decompress only when the user asked (`--compressed`).
   - Go **discards HEAD response bodies**; the reference read them (and its
     evaluation layer *synthesized* `{"method":"HEAD"}` for empty bodies).
     Had to replicate the synthesis.
   - `-X METHOD` vs implied method: data implies POST **only without an
     explicit method** — the port overwrote `PATCH` on the first run.
8. **Numeric identity in expression engines.** go-jmespath compares with
   `reflect.DeepEqual`: TOML `int64(200)` ≠ JSON-literal `float64(200)`.
   The port normalizes every number to float64 at the evaluation boundary.
   Expect the equivalent in any rule/expression engine you swap.

### Harness and tooling traps

9. **Self-mismatching harness.** First parity run: 0/150 — mostly harness
   bugs (raw-JSON comparison, a generator missing a `\` line continuation,
   `json.dumps` emitting `\uXXXX` surrogate escapes that are invalid TOML).
   The port was more right than the harness. Self-check + probe before
   believing a mismatch, in that order.
10. **Stale fixture state looks like a regression.** A leftover readiness
    file sent 320 cases to a dead server port (175/495). Delete or version
    every fixture artifact between runs.
11. **Build the fixture server first and prove it byte-identical.** If the
    fixture itself drifts, every divergence after that is unattributable.
12. **cwd-relative pathspecs lie** (`git ls-files go/` from inside `go/`
    silently returns nothing — briefly convinced us files were missing), and
    **gitignore patterns over-match** (`mdok` in `go/.gitignore` matched the
    source directory `cmd/mdok`; scope release-artifact patterns with a
    leading `/`).
13. **Distributed binaries need `--version`** wired before release, and the
    install path (brew formula test block) will exercise it.

### Distribution traps

14. **Brew formula heredocs**: `sha256 "#{VAR}"` in a shell heredoc passes
    through unexpanded, and Ruby then evaluates `VAR` as an uninitialized
    constant at install time. Inline digests with a script, never by hand.
15. **Brew caches tap formulas**: after pushing a fix, force-refresh the
    local tap (`git -C $(brew --repository owner/tap) reset --hard
    origin/main`) or you'll debug the old formula.
16. **Prebuilt binaries vs build-from-source formula**: switching the tap
    from a cargo build-from-source formula (~10 min installs) to release
    archives is a big UX win, but note the old formula's capability (it
    installed the full-feature Rust binary) in the release notes.

## 6. Reusable checklist

- [ ] Inventory tests: portable vs white-box; make runners `--binary`-aware
- [ ] Measure portable-suite coverage; list spec-vs-implementation gaps
- [ ] Fix test gaps in the Rust repo while it is the reference
- [ ] Port the fixture/test server; prove it byte-identical to the original
- [ ] Freeze shared contract types; pre-install all dependencies
- [ ] Fan out leaf packages; coordinator owns integration + CLI
- [ ] Pass the existing e2e suite with the new binary
- [ ] Build the differential parity suite: deterministic generation, shared
      fixture, side-keyed state, projected comparison, self-check mode
- [ ] Iterate: probe the reference for every mismatch; never "fix" toward a
      guess
- [ ] Port the exit-code taxonomy and diagnostic shapes explicitly
- [ ] Cross-compile all platforms; smoke-test the actual artifact through
      the e2e suite
- [ ] Release; update the tap; install for real; run e2e via the installed
      binary
- [ ] Keep parity (and e2e) as permanent CI gates for *both* implementations

The single most transferable lesson: **the reference binary is the spec.**
When in doubt, run it, capture its exact output, and make the port reproduce
that — not your mental model of it.
