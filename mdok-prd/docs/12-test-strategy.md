# 12. Test Strategy

## 12.1 Layers

1. **Parser unit tests:** Markdown extraction, fence metadata, template grammar, Bash AST restrictions, JMESPath compilation.
2. **C unit tests:** curl parser bridge, option policy, ownership, callbacks, allocation failures.
3. **Differential tests:** compare accepted argv behavior and generated transfer characteristics with the pinned curl executable.
4. **Integration tests:** run corpus documents against the deterministic local HTTP/HTTPS server.
5. **Golden diagnostics:** exact structured error JSON and stable code/span assertions.
6. **Property tests:** templates cannot alter argv cardinality; parse/format invariants; secret redaction.
7. **Fuzzing:** Markdown, info strings, templates, Bash source, argv-to-C FFI, headers, and response bodies.
8. **Sanitizers:** ASan, UBSan, LSan, and TSan where supported.
9. **Benchmarks:** parse, plan, JMESPath, transfer setup, body capture, and report generation.
10. **Cross-platform CI:** macOS, Linux, Windows; arm64 jobs where available.

## 12.2 Corpus

This bundle includes 495 `.md` tests under `tests/corpus/`. `index.jsonl` is authoritative and contains expected stage/outcome/error code. Tests are deterministic and use only local fixture-server endpoints.

The corpus covers:

- valid and invalid Markdown/fence metadata;
- restricted shell syntax and injection attempts;
- HTTP methods, headers, bodies, forms, files, auth, cookies, redirects, TLS, proxy/DNS controls;
- curl compatibility and explicitly rejected semantics;
- JMESPath pass/fail/type/parse cases;
- typed captures and chained variables;
- template filters and secret taint;
- body/header/binary models;
- timeouts, retries, cancellation, limits, reports, and ordering.

It is broad but not mathematically exhaustive. CI must additionally generate one policy test for every option in the pinned curl source and fail on unclassified options.

## 12.3 Fixture server

`mdok-test-server` listens on dynamically assigned loopback HTTP and HTTPS ports and prints a JSON readiness record. It supports endpoint contracts in `docs/17-fixture-server.md`. Tests receive `base_url`, `https_base_url`, `proxy_url`, and fixture paths through harness variables.

## 12.4 Test naming

```text
<category>/<NNN>-<short-name>.md
```

IDs are stable even if files move. New regression tests use a new ID rather than repurposing an old fixture.

## 12.5 Acceptance gate

A release candidate requires:

- all corpus tests passing on primary targets;
- no sanitizer findings;
- no new unclassified curl options;
- JMESPath compliance suite passing;
- CommonMark/GFM relevant suites passing;
- benchmark regression within allowed thresholds;
- deterministic JSON report snapshots.
