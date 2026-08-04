# 15. Roadmap and Acceptance Criteria

## Phase 0 — curl integration spike

Deliver:

- pinned curl source fetch and verification;
- C patch exposing parser/plan/execute bridge;
- Rust FFI smoke program;
- GET/POST/header/body/redirect/TLS examples on macOS, Linux, Windows;
- parser differential test against bundled curl.

Exit criteria: no process exits from bridge, leak-free repeated parse/execute/free, and a documented option-classification extraction path.

## Phase 1 — parser and planner

Deliver Markdown AST extraction, fence metadata parser, TOML vars, Bash AST restrictions, template AST, JMESPath compilation, source spans, `lint`, `list`, and `plan`.

Exit criteria: all parse/plan corpus cases pass without network access.

## Phase 2 — sequential runtime

Deliver HTTP/HTTPS execution, response model, checks, captures, variables, connection reuse, local fixture server, human and JSON reports.

Exit criteria: all basic execution, JMESPath, capture, and workflow corpus cases pass.

## Phase 3 — security and completeness

Deliver host/IP/filesystem/proxy/TLS policies, secret taint/redaction, limits, cancellation, retries, redirects, binary/spooling, and complete curl option classification.

Exit criteria: security corpus, sanitizers, fuzz smoke, and option policy gates pass.

## Phase 4 — CI quality

Deliver JUnit, JSON Lines events, parallel documents, caching of compiled expressions, benchmark gates, cross-platform release pipeline, SBOM, and signed checksums.

Exit criteria: release checklist is fully automated except signing authorization.

## Version 1.0 acceptance

- 100% of required functional requirements implemented.
- 495 bundled corpus tests plus generated per-curl-option policy tests pass.
- CommonMark/GFM relevant tests and upstream JMESPath compliance tests pass.
- ASan/UBSan/LSan clean; TSan clean for supported concurrency tests.
- Fuzz targets complete a minimum CI smoke budget with no crash.
- No known critical/high security issue.
- Performance targets met or exceptions documented and approved.
- Installation and local implementation instructions validated on a clean Mac, Linux VM/container, and Windows runner.
