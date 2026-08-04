# 1. User Stories and Requirements

## 1.1 Core stories

### Author and run

As a developer, I can paste a valid curl command into a Markdown fence, name it, add JMESPath checks, and run the document with `mdok file.md`.

### Chain requests

As a developer, I can capture an object from one response with JMESPath and reference its fields in later curl arguments.

### Repair with an agent

As an AI agent, I receive stable JSON diagnostics containing the failing file, step, expression, source span, observed value, redacted context, and suggested category of repair.

### Review in Git

As a reviewer, I can read a useful API narrative without running MDOK or installing an editor plugin.

### Execute safely

As an operator, I can restrict schemes, hosts, ports, filesystem access, redirects, body sizes, and secrets before running untrusted documents.

### Run in CI

As a CI system, I receive deterministic exit codes, JSON/JUnit reports, and no interactive prompts.

## 1.2 Functional requirements

| ID | Requirement | Priority |
|---|---|---|
| FR-001 | Parse CommonMark 0.31.2-compatible Markdown plus GFM extensions. | Must |
| FR-002 | Preserve byte and line/column spans for executable fences and expressions. | Must |
| FR-003 | Recognize `toml mdok vars`, `curl mdok name=...`, `jmespath mdok check=...`, and `jmespath mdok capture=...`. | Must |
| FR-004 | Parse a curl fence as exactly one restricted shell simple command. | Must |
| FR-005 | Reject pipes, redirects, command substitution, parameter expansion, arithmetic expansion, process substitution, backgrounding, and multiple commands. | Must |
| FR-006 | Use curl's actual tool parser for command options. | Must |
| FR-007 | Execute transfers with a pinned libcurl build. | Must |
| FR-008 | Disable implicit `.curlrc` loading. | Must |
| FR-009 | Enforce one logical transfer per curl fence in version 1. | Must |
| FR-010 | Support HTTP and HTTPS by default; gate WS/WSS separately; deny other schemes. | Must |
| FR-011 | Evaluate every check line as standard JMESPath and require boolean `true`. | Must |
| FR-012 | Evaluate a capture fence as one JMESPath expression returning an object. | Must |
| FR-013 | Provide typed template filters for string, JSON, URL, header, base64, and raw insertion. | Must |
| FR-014 | Redact secret-derived values transitively in outputs. | Must |
| FR-015 | Emit human, JSON, and JUnit reports. | Must |
| FR-016 | Provide `test`, `lint`, `plan`, and `list` modes. | Must |
| FR-017 | Support files, directories, globs, and stdin. | Should |
| FR-018 | Reuse connections within a run. | Must |
| FR-019 | Run documents sequentially by default; optionally parallelize independent files. | Should |
| FR-020 | Provide deterministic local fixture server for integration tests. | Must |

## 1.3 Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-001 | No undefined behavior across the C/Rust FFI boundary under sanitizers. |
| NFR-002 | No Rust panic or C `longjmp` crosses the FFI boundary. |
| NFR-003 | All allocations derived from untrusted input are limited or fallible. |
| NFR-004 | Body capture spills to disk above a configurable threshold. |
| NFR-005 | Cancellation reaches libcurl promptly through progress callbacks. |
| NFR-006 | Reproducible release artifacts include an SBOM and curl provenance. |
| NFR-007 | Backward compatibility is versioned by the MDOK language version and curl compatibility version. |
| NFR-008 | Tests run without public internet access. |
