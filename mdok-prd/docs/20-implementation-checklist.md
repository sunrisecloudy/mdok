# 20. Implementation Checklist

Use this as the progress ledger. A phase is complete only when its acceptance tests and quality gates are checked.

## A. Repository and build foundation

- [ ] Create the Rust workspace exactly once; keep crate dependency direction acyclic.
- [ ] Pin the Rust toolchain and commit `Cargo.lock`.
- [ ] Add CMake/Ninja native build integration through `mdok-curl-sys`.
- [ ] Add warning-as-error profiles for Rust and C in CI.
- [ ] Add formatting, linting, unit-test, integration-test, sanitizer, fuzz-smoke, and benchmark jobs.
- [ ] Add a third-party notice and automated SBOM generation.
- [ ] Add reproducible source archive and binary packaging scripts.
- [ ] Embed MDOK, curl, libcurl, TLS backend, and feature versions.

## B. curl source and C bridge spike

- [ ] Download curl 8.21.0 from the official release source.
- [ ] Verify and commit the official checksum metadata.
- [ ] Preserve curl's COPYING and notices.
- [ ] Build unmodified curl and libcurl on all Tier 1 targets.
- [ ] Identify the smallest set of curl tool source files needed for option parsing.
- [ ] Replace tool fatal exits with returned errors in a minimal patch.
- [ ] Export parse, plan, execute, and free entry points behind `mdok_curl.h`.
- [ ] Ensure the bridge hides curl internal symbols and structures.
- [ ] Inject `-q` before user argv so `.curlrc` is not loaded implicitly.
- [ ] Prohibit interactive prompts.
- [ ] Add C allocation-failure hooks for tests.
- [ ] Run parse/free loops under ASan and LSan.
- [ ] Run malformed argv under UBSan.
- [ ] Prove cancellation through `CURLOPT_XFERINFOFUNCTION`.
- [ ] Prove response header/body callbacks support short writes and cancellation.
- [ ] Prove easy-handle reset prevents method/header/body/auth leakage between steps.
- [ ] Produce a real patch file replacing the placeholder.

## C. Curl option inventory and policy

- [ ] Generate the complete long-option inventory from vendored `tool_listhelp.c`.
- [ ] Map aliases and short options to canonical long options.
- [ ] Classify every option as transfer, compatibility-noop, virtualized, policy-gated, unsupported, or protocol-denied.
- [ ] Fail CI on every unclassified option.
- [ ] Add one generated parse/policy test for every option.
- [ ] Add differential tests against the bundled curl executable.
- [ ] Test repeated, negated, reset, and `--no-*` options.
- [ ] Test missing and malformed option arguments.
- [ ] Test feature-gated options against builds with and without each feature.
- [ ] Reject multiple URLs, URL glob expansion, `--next`, and `--parallel` in version 1.
- [ ] Recheck all resolved URLs after redirects and connect overrides.

## D. Markdown parser

- [ ] Configure Comrak for CommonMark 0.31.2 and required GFM behavior.
- [ ] Parse UTF-8 with BOM handling and source-size limits.
- [ ] Walk fenced-code AST nodes without regex fence detection.
- [ ] Preserve byte spans, line/column, heading hierarchy, and original info strings.
- [ ] Ignore non-MDOK fences exactly.
- [ ] Parse info strings with a dedicated grammar and source spans.
- [ ] Validate languages, roles, required attributes, duplicates, and unknown attributes.
- [ ] Parse inline TOML variable tables.
- [ ] Validate step names, uniqueness, reference order, and reserved names.
- [ ] Run relevant CommonMark/GFM test vectors.
- [ ] Fuzz Markdown and fence metadata parsers.

## E. Template parser and typed values

- [ ] Implement the template grammar as a parser, not global replacement.
- [ ] Preserve template source spans.
- [ ] Resolve nested object/array paths.
- [ ] Implement `string`, `raw`, `json`, `url`, `header`, and `base64` filters.
- [ ] Reject object/array values for scalar filters.
- [ ] Reject CR/LF in header-filtered values.
- [ ] Enforce per-template and total expansion limits.
- [ ] Keep inserted values as data; never parse them as shell source.
- [ ] Add typed variable precedence and duplicate-definition checks.
- [ ] Add secret source declarations and no-prompt CI behavior.
- [ ] Implement conservative secret taint and redacted formatting.
- [ ] Property-test that interpolation never changes argv cardinality.
- [ ] Fuzz template syntax and expansion boundaries.

## F. Restricted curl fence shell parser

- [ ] Mask or tokenize MDOK templates before Bash parsing while preserving source mapping.
- [ ] Parse with Tree-sitter Bash.
- [ ] Accept exactly one simple command named `curl`.
- [ ] Evaluate literal, single-quoted, double-quoted, escaped, and continued word segments.
- [ ] Reject pipes, lists, redirects, substitutions, assignments, functions, subshells, backgrounding, and extra commands.
- [ ] Reject ordinary shell parameter expansion and arithmetic expansion.
- [ ] Reject ambiguous/invalid Bash parse trees.
- [ ] Build exact UTF-8 argv values.
- [ ] Ensure secret argv data is not logged.
- [ ] Fuzz shell source and AST traversal.

## G. Planner

- [ ] Parse all selected documents before executing any request in that document.
- [ ] Build immutable `DocumentPlan` and `StepPlan` structures.
- [ ] Compile JMESPath expressions during planning.
- [ ] Validate checks/captures reference earlier requests.
- [ ] Validate capture-key availability and collisions.
- [ ] Validate all static curl option policies.
- [ ] Produce a redacted normalized plan for `mdok plan`.
- [ ] Produce stable source-located diagnostics.
- [ ] Cache plans only with content/config/curl-version keys.

## H. libcurl runtime

- [ ] Initialize libcurl exactly once.
- [ ] Create one execution session/multi handle per worker.
- [ ] Resolve templates and call the C parser with final argv.
- [ ] Apply runtime URL/IP/path/proxy/TLS policies.
- [ ] Configure body and header callbacks.
- [ ] Capture all duplicate headers in receive order.
- [ ] Record redirect hops and final response separately.
- [ ] Record integer-microsecond timings where libcurl provides them.
- [ ] Record transfer sizes, addresses, ports, HTTP version, and proxy use.
- [ ] Decode compressed content consistently.
- [ ] Reuse connections and test state reset.
- [ ] Implement timeout, retry, low-speed, and cancellation behavior.
- [ ] Handle early close, partial body, malformed headers, and callback failures.
- [ ] Ensure every C handle and buffer has one clear owner and cleanup path.

## I. Body storage and response model

- [ ] Buffer small bodies in memory.
- [ ] Spill above threshold to owner-only temporary files.
- [ ] Enforce absolute body and header limits during callbacks.
- [ ] Detect empty, JSON, text, binary, and truncated bodies.
- [ ] Preserve exact bytes independently from decoded/parsed representations.
- [ ] Bound JSON parse depth and size.
- [ ] Expose the response object exactly as specified.
- [ ] Lowercase header names and retain arrays of values.
- [ ] Ensure body/report formatting cannot expose tainted secrets.
- [ ] Delete temporary files on success, failure, cancellation, and process-cleanup paths.

## J. JMESPath checks and captures

- [ ] Integrate the selected Rust JMESPath implementation.
- [ ] Run the upstream JMESPath compliance suite.
- [ ] Cache compiled expressions.
- [ ] Evaluate each check line independently and require boolean `true`.
- [ ] Distinguish parse, runtime/type, and false-result errors.
- [ ] Evaluate capture blocks as one expression returning an object.
- [ ] Preserve captured JSON types.
- [ ] Publish captures only after source checks pass.
- [ ] Detect invalid/colliding capture keys.
- [ ] Include variables and prior-step summaries in evaluation context.
- [ ] Bound JMESPath output size and evaluation resources.

## K. Security policy

- [ ] Implement exact/wildcard hostname rules.
- [ ] Implement CIDR rules for resolved and connected addresses.
- [ ] Deny cloud metadata/link-local targets by default outside local-test mode.
- [ ] Revalidate every redirect hop.
- [ ] Validate `--resolve` and `--connect-to` against final connection targets.
- [ ] Normalize/canonicalize file paths without symlink escape.
- [ ] Deny device paths, FIFOs, stdin bodies, and unsafe special files.
- [ ] Gate proxies and Unix sockets.
- [ ] Gate `--insecure` and client certificate files.
- [ ] Enforce request/upload/header/body/redirect/retry/time limits.
- [ ] Add attack-focused corpus tests and fuzz seeds.
- [ ] Document the untrusted-document execution model.

## L. CLI and reporting

- [ ] Implement `mdok PATH` alias and `test`, `lint`, `plan`, `list`, `version`.
- [ ] Implement project discovery and `.gitignore`-aware file discovery.
- [ ] Implement environment/profile and CLI variable overrides.
- [ ] Implement stable exit codes.
- [ ] Implement concise human output.
- [ ] Implement JSON report schema version 1.
- [ ] Implement JSON Lines event streaming with sequence numbers.
- [ ] Implement atomic report writes.
- [ ] Implement JUnit output.
- [ ] Redact secrets in every formatter and debug path.
- [ ] Test broken pipes and interrupted output.

## M. Fixture server and corpus harness

- [ ] Implement every endpoint in `docs/17-fixture-server.md`.
- [ ] Bind only to loopback and dynamic ports.
- [ ] Generate deterministic local CA/leaf certificates.
- [ ] Print the readiness JSON contract exactly.
- [ ] Namespace mutable retry/cookie state per test key.
- [ ] Load and validate `tests/corpus/index.jsonl`.
- [ ] Run plan-only cases without starting the server.
- [ ] Inject fixture paths and URLs as harness variables.
- [ ] Assert expected code, stage, span, and outcome.
- [ ] Snapshot JSON diagnostics and reports.
- [ ] Run all 495 bundled Markdown fixtures.
- [ ] Add generated tests for every pinned curl option.
- [ ] Add every discovered bug as a permanent regression fixture.

## N. Quality gates

- [ ] Rust unit/integration/doc tests pass.
- [ ] C unit tests pass.
- [ ] CommonMark/GFM relevant suites pass.
- [ ] JMESPath compliance suite passes.
- [ ] Curl differential suite passes.
- [ ] All corpus tests pass on Tier 1 targets.
- [ ] ASan, UBSan, and LSan report no issue.
- [ ] TSan reports no concurrency issue where supported.
- [ ] Fuzz smoke runs complete without crashes.
- [ ] Benchmarks meet the approved regression budget.
- [ ] Peak memory and file-descriptor limits are measured.
- [ ] Secret scans find no fixture or report leakage.
- [ ] SBOM and license checks pass.

## O. Release

- [ ] Produce clean-checkout release builds.
- [ ] Test archive extraction and `mdok version` on clean systems.
- [ ] Run local HTTP and HTTPS smoke files.
- [ ] Publish checksums, SBOM, provenance, and third-party notices.
- [ ] Sign authorized artifacts/checksums.
- [ ] Record exact curl/libcurl/TLS features for every artifact.
- [ ] Preserve prior language/report/error compatibility fixtures.
- [ ] Complete the version 1 acceptance criteria.
