# 0. Product Requirements Document

## 0.1 Product statement

MDOK is a local, CLI-first, AI-agent-native workflow testing tool. A test is a normal Markdown document containing executable curl or trusted direct-command fences, JMESPath checks, and JMESPath captures. The document is simultaneously documentation, test code, a reproducible API/tool example, and an agent-readable repair target.

The primary question is: **"Is this Markdown still okay?"**

## 0.2 Problem

API knowledge is commonly split across Markdown documentation, curl snippets, Postman/Bruno collections, integration test code, CI configuration, and support tickets. These representations drift. They are hard for humans to review together and force AI agents to translate between formats.

MDOK makes the Markdown example itself executable without replacing curl or inventing an HTTP DSL.

## 0.3 Goals

1. Execute copied curl commands consistently without requiring a system curl executable.
2. Parse Markdown with a standards-compliant parser and preserve source spans.
3. Use curl's actual command-line option parser in C and libcurl for transfers.
4. Use strict standard JMESPath for all response checks and captures.
5. Support chained requests through named, typed variables.
6. Produce human-readable and machine-readable diagnostics suitable for autonomous repair loops.
7. Be deterministic, fast, memory-bounded, cross-platform, and safe by default.
8. Run locally and in CI with no required cloud account.
9. Let agents store deterministic, reviewable command tests for repository-local tools without granting shell access.

## 0.4 Users

- Developers reviewing API examples in Git.
- AI coding agents generating and repairing integration tests.
- QA engineers expressing API workflows without a proprietary collection format.
- Documentation teams continuously verifying examples.
- Support and operations teams sharing one-file reproductions.

## 0.5 Version 1 use cases

- Login, capture a token, and call an authenticated endpoint.
- CRUD workflow with captured resource IDs.
- API documentation verification in CI.
- Deployment smoke tests.
- Reproducible customer issue files.
- Matrix execution against development, staging, and production-safe profiles.
- Local fixture-server testing with redirects, cookies, TLS, compression, binary bodies, and failures.
- Repository-local agent/tool validation commands stored as versioned Markdown.

## 0.6 Version 1 non-goals

- General shell scripting.
- Arbitrary external process execution or ambient `PATH` lookup.
- Browser/UI automation.
- Load testing or distributed performance testing.
- OpenAPI generation as a core runtime feature.
- Full support for curl's non-HTTP protocols.
- A promise that every future curl option is immediately executable.
- Graphical editing or hosted collaboration.

## 0.7 Success metrics

- A new user can author a login-and-profile flow in under ten minutes using familiar curl.
- At least 95% of curl snippets copied from typical REST API documentation parse without edits when they use supported transfer semantics.
- Unsupported behavior always fails before network execution with a source-located explanation.
- Cold CLI startup under 50 ms on a modern developer laptop, excluding dynamic-loader variance.
- Parse-and-plan 1,000 small MDOK documents in under one second on a modern 8-core machine.
- Bounded body capture and no unbounded in-memory buffering.
- Zero plaintext secret values in normal diagnostics, JSON reports, or crash-safe logs.
- The compatibility corpus passes on macOS arm64/x86_64, Linux x86_64/aarch64, and Windows x86_64.

## 0.8 Product principles

- **Use established languages:** CommonMark/GFM, curl, TOML, JMESPath.
- **No silent semantic loss:** accept, reject, or explicitly virtualize each curl option.
- **Source location everywhere:** every plan item and failure traces to the Markdown file.
- **Parse, do not grep:** no regex-based Markdown or shell interpretation.
- **Safe interpolation:** variable values are never re-parsed as shell syntax.
- **Structured core, friendly surface:** all execution produces a stable typed result before formatting.
- **Offline first:** execution needs only the binary, documents, referenced files, and target API.
