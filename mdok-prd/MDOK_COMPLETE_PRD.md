# MDOK Complete PRD and Implementation Specification

This generated file combines the detailed documents in `docs/`. The split files remain authoritative for focused editing.


---

# 0. Product Requirements Document

## 0.1 Product statement

MDOK is a local, CLI-first, AI-agent-native workflow testing tool. A test is a normal Markdown document containing executable curl fences or trusted direct-command `exec` fences, JMESPath checks, and JMESPath captures. The document is simultaneously documentation, test code, a reproducible workflow example, and an agent-readable repair target.

The primary question is: **"Is this Markdown still okay?"**

## 0.2 Problem

API knowledge is commonly split across Markdown documentation, curl snippets, Postman/Bruno collections, integration test code, CI configuration, and support tickets. These representations drift. They are hard for humans to review together and force AI agents to translate between formats.

MDOK makes the Markdown example itself executable without replacing curl or inventing a shell DSL. Curl remains the default HTTP source; agents can also store and replay approved non-shell commands through named executable profiles.

## 0.3 Goals

1. Execute copied curl commands consistently without requiring a system curl executable.
2. Parse Markdown with a standards-compliant parser and preserve source spans.
3. Use curl's actual command-line option parser in C and libcurl for transfers.
4. Use strict standard JMESPath for all response checks and captures.
5. Support chained requests through named, typed variables.
6. Produce human-readable and machine-readable diagnostics suitable for autonomous repair loops.
7. Be deterministic, fast, memory-bounded, cross-platform, and safe by default.
8. Run locally and in CI with no required cloud account.
9. Let AI agents retain deterministic command tests for fixture tools, formatters, probes, and other approved executables.

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

## 0.6 Version 1 non-goals

- General shell scripting.
- Ambient `PATH` lookup, arbitrary child processes, and shell interpreter execution.
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


---

# 1. User Stories and Requirements

## 1.1 Core stories

### Author and run

As a developer, I can paste a valid curl command into a Markdown fence, name it, add JMESPath checks, and run the document with `mdok file.md`.

### Store and replay an approved command

As an AI agent, I can store a direct-argv command test in Markdown, bind it to a trusted executable profile, add checks over bounded output, and run it again later without reconstructing shell state.

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
| FR-021 | Recognize `exec mdok name=...` fences containing one direct-argv command. | Must |
| FR-022 | Resolve executable commands only through explicitly configured canonical profiles; never use ambient `PATH`. | Must |
| FR-023 | Clear inherited child environment and allow only configured non-secret and secret environment mappings. | Must |
| FR-024 | Apply bounded timeout, argument, combined-output, and process-group limits to external commands. | Must |
| FR-025 | Expose bounded stdout/stderr and exit metadata to checks while omitting command output from durable reports. | Must |

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


---

# 2. MDOK Language Specification

## 2.1 File recognition

Any UTF-8 Markdown file may contain MDOK blocks. `.mdok.md` is recommended but not required. Invalid UTF-8 is a parse error. A UTF-8 BOM is accepted and excluded from source columns.

## 2.2 Executable fence forms

### Variables

````markdown
```toml mdok vars
base_url = "http://127.0.0.1:9800"
user = "alice"
```
````

The content is parsed by a TOML parser. The resulting root must be a table. Inline variables are document-scoped and immutable after planning. Captures live in a separate runtime namespace.

### Request

````markdown
```curl mdok name=create_user
curl --request POST "{{base_url}}/users" \
  --header "Content-Type: application/json" \
  --data-raw '{"name":{{user|json}}}'
```
````

`name` is required and must be unique in a document. A request fence contains exactly one curl simple command. The leading word must be `curl`; omitting it is not supported in version 1 because copied commands should remain executable outside MDOK.

### Trusted external command

````markdown
```exec mdok name=inspect_fixture
mdok-command-fixture json
```
````

An `exec` fence is one direct-argv command, not a shell program. Its first token must match a configured `[policy.exec.commands.<name>]` profile whose `program` is an absolute canonical executable path. Shell interpreters, shell operators, ambient `PATH` lookup, inherited environment variables, and unapproved working directories are rejected during planning or configuration loading. Command output is bounded and available transiently to checks/captures through the execution context; durable reports retain only redacted argv, status, limit, byte-count, and timing metadata.

### Checks

````markdown
```jmespath mdok check=create_user
status == `201`
body.name == variables.user
length(headers."content-type") == `1`
```
````

Each non-empty line is one complete standard JMESPath expression. Each expression must evaluate to boolean `true`. Blank lines are ignored. JMESPath comments are not invented; explanation belongs in surrounding Markdown.

### Capture

````markdown
```jmespath mdok capture=create_user
{id: body.id, etag: headers.etag[0]}
```
````

A capture fence contains one complete JMESPath expression. The result must be an object. Each top-level key becomes a captured variable after all checks associated with the source step have passed. Null values are allowed unless project policy forbids them.

## 2.3 Fence metadata grammar

The CommonMark info string is parsed after Markdown parsing. It uses a restricted argument grammar:

```ebnf
info-string   = language, 1*space, "mdok", *(1*space, attribute) ;
language      = identifier ;
attribute     = flag | key, "=", value ;
flag          = identifier ;
key           = identifier ;
value         = bare-value | single-quoted | double-quoted ;
identifier    = letter, *(letter | digit | "_" | "-") ;
bare-value    = 1*(unreserved) ;
```

Duplicate attributes, unknown required attributes, malformed quoting, and conflicting block roles are planning errors.

## 2.4 Step identifiers

```text
^[A-Za-z][A-Za-z0-9_-]{0,63}$
```

Identifiers are case-sensitive. Reserved names include `variables`, `steps`, `environment`, `request`, `response`, and `mdok`.

## 2.5 Association and order

- Requests execute in document order.
- A check or capture may refer only to a request step defined earlier in the document in version 1.
- Multiple check fences may target one step; their expressions are evaluated in source order.
- Multiple capture fences may target one step; keys must not collide unless `allow_capture_override=true` is explicitly configured.
- Captures become available only after the source step's transfer and checks succeed.
- A request that references an unavailable capture fails before network execution.

## 2.6 Ignored Markdown

Normal prose, headings, lists, tables, links, images, HTML, inline code, and code fences without the `mdok` marker are not executed. They remain part of source context for diagnostics.

## 2.7 Language version

`mdok.toml` declares the language version:

```toml
language = "1"
curl_compat = "8.21"
```

A document may override only with an explicit HTML metadata comment in a future version; version 1 uses project configuration to avoid front-matter ambiguity.


---

# 3. curl Command Parsing and Compatibility

## 3.1 Design

MDOK does not create an HTTP command. It accepts a real curl command, parses its shell structure safely in Rust, passes the resulting `argv` to curl's real tool parser in C, and executes the resulting transfer through libcurl.

```text
Markdown AST
  -> restricted Bash AST
  -> template-aware argv
  -> curl tool parser (C)
  -> validated transfer plan
  -> libcurl multi interface
```

## 3.2 Restricted shell grammar

The curl fence is parsed with Tree-sitter Bash. Version 1 accepts exactly:

- one `command` / simple-command node;
- command name `curl`;
- ordinary words made from literal, single-quoted, and double-quoted segments;
- backslash escaping and backslash-newline continuation;
- MDOK template expressions embedded in word segments.

It rejects before interpolation:

- `|`, `|&`, `&&`, `||`, `;`, newline-separated commands;
- redirections including `>`, `<`, `2>`, here-documents, and here-strings;
- command substitution `$(...)` and backticks;
- shell variables `$x`, `${x}`, special parameters, and arithmetic expansion;
- process substitution `<(...)` and `>(...)`;
- glob expansion, brace expansion, tilde expansion, aliases, functions, assignments, and subshells;
- background execution `&`.

Template values are inserted into already-parsed word segments and are never evaluated as shell source.

## 3.3 curl parser integration

A pinned curl source release is vendored. A small maintained patch exposes the tool parser behind an MDOK-owned C API. MDOK must not include curl internal structs in Rust. The bridge translates tool parser output into an MDOK transfer plan and configures libcurl.

The patch must remain small, reviewable, and separately stored under `vendor/patches/curl/`. Every curl upgrade runs differential parser tests against the bundled curl executable.

## 3.4 Determinism changes

MDOK intentionally changes or constrains these curl-tool behaviors:

- Inject `-q` before user arguments so an implicit `.curlrc` is never loaded.
- One logical transfer per request fence; reject `--next`, multiple URLs, URL glob expansion, and `--parallel` in version 1.
- Default allowed schemes are `http` and `https`.
- Interactive prompts are disabled.
- Terminal formatting and progress output are replaced by structured reporting.
- Filesystem reads and writes pass through MDOK policy checks.
- Standard input is unavailable inside a curl fence except explicit MDOK-provided body input in a future version.

## 3.5 Option classifications

Every curl option in the pinned release receives exactly one classification:

1. **transfer** — preserved and executed through libcurl;
2. **compatibility-noop** — accepted because it only affects curl terminal presentation, with documented MDOK behavior;
3. **virtualized** — preserved through an MDOK abstraction such as artifact output;
4. **policy-gated** — available only when permissions allow it;
5. **unsupported** — parsed, then rejected with an exact reason;
6. **protocol-denied** — valid curl behavior outside MDOK's allowed protocols.

Silent ignoring is prohibited.

## 3.6 Version 1 support baseline

Transfer semantics targeted for version 1 include:

- methods: GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS, custom methods;
- headers and header files;
- JSON/raw/form-urlencoded/multipart/binary bodies and uploads;
- Basic, Bearer, Digest, Negotiate where compiled, AWS SigV4 where compiled;
- cookies and cookie engine;
- redirects with redirect limits;
- timeouts, low-speed limits, retries, retry delay/max-time;
- TLS verification, CA files/paths, client certificates, ciphers, TLS versions;
- HTTP/1.0, HTTP/1.1, HTTP/2, optional HTTP/3 builds;
- proxies, NO_PROXY, connect-to, resolve, Unix sockets where available;
- compressed responses, ranges, conditional requests, ETags;
- request and response size limits enforced by MDOK.

## 3.7 Explicitly unsupported in version 1

- non-HTTP protocols;
- multiple transfers per fence;
- `--parallel` and parallel-immediate modes;
- terminal/UI controls that cannot affect transfer semantics;
- remote-name/output file behaviors unless mapped to an MDOK artifact in a later release;
- `--libcurl`, trace files, and config generation;
- stdin-driven bodies and password prompts;
- options requiring an unavailable build feature.

## 3.8 Compatibility manifest

`scripts/sync_curl_options.py` reads `vendor/curl/src/tool_listhelp.c` after vendoring and generates `specs/curl-option-policy.csv`. CI fails when a curl upgrade adds an unclassified option.


---

# 4. JMESPath and Structured Transfer Results

## 4.1 Strict standard JMESPath

Version 1 uses the standard JMESPath grammar and built-in functions. MDOK does not silently add regex, date, schema, or assertion functions. Expressions are compiled during planning and reused at execution.

## 4.2 Check semantics

For each non-empty check line:

- parse failure -> `MDOK-E500`;
- runtime/type failure -> `MDOK-E501`;
- result `true` -> pass;
- result `false` -> `MDOK-E502` assertion failure;
- result `null` or any non-boolean -> `MDOK-E501` type failure.

Checks are not skipped because of later checks. By default all checks for a completed step run, so one response can report multiple failures. `--fail-fast` changes this.

## 4.3 Capture semantics

A capture fence is compiled as one expression. It must evaluate to an object:

```jmespath
{token: body.access_token, ids: body.items[].id}
```

Non-object results fail with `MDOK-E503`. Keys must be valid variable names. Captured arrays and objects remain typed in the variable store.

## 4.4 Evaluation object

Every check and capture sees this stable object:

```json
{
  "status": 200,
  "method": "POST",
  "url": "https://api.example.test/login",
  "effective_url": "https://api.example.test/login",
  "http_version": "2",
  "headers": {
    "content-type": ["application/json"],
    "set-cookie": ["a=1; Path=/", "b=2; Path=/"]
  },
  "body": {"access_token": "secret", "user": {"id": "u_1"}},
  "body_text": "{...}",
  "body_base64": null,
  "body_kind": "json",
  "cookies": [{"name": "a", "value": "1", "domain": "api.example.test"}],
  "redirects": [],
  "timings": {
    "queue_ms": 0.0,
    "dns_ms": 1.2,
    "connect_ms": 3.4,
    "tls_ms": 12.0,
    "ttfb_ms": 20.1,
    "total_ms": 21.4,
    "redirect_ms": 0.0
  },
  "transfer": {
    "uploaded_bytes": 54,
    "downloaded_bytes": 91,
    "request_header_bytes": 245,
    "response_header_bytes": 188,
    "primary_ip": "127.0.0.1",
    "primary_port": 9800,
    "local_ip": "127.0.0.1",
    "local_port": 53124,
    "redirect_count": 0,
    "used_proxy": false
  },
  "tls": {
    "verified": true,
    "verify_result": 0
  },
  "error": null,
  "variables": {
    "email": "agent@example.com",
    "user_id": "u_1"
  },
  "steps": {
    "login": {"status": 200}
  }
}
```

## 4.5 Header model

Header names are ASCII-lowercased. Values are arrays preserving receive order. Duplicate headers are never comma-joined because that is invalid for fields such as `set-cookie`. Interim `1xx` response headers and redirect-hop headers are retained in `redirects`; `headers` represents the final response.

## 4.6 Body model

- Valid JSON is exposed as typed `body`; `body_kind="json"`.
- Valid UTF-8 non-JSON text is exposed as string `body` and `body_text`; `body_kind="text"`.
- Binary data has `body=null`, `body_text=null`, and optional bounded `body_base64`; `body_kind="binary"`.
- Empty body has `body=null`, `body_text=""`, and `body_kind="empty"`.
- JSON parsing uses bytes after content decoding performed by libcurl.
- A policy controls whether content type is required for JSON detection; default is content type or successful JSON parse.

## 4.7 Secret taint

JMESPath evaluation operates on real values, but reporting wraps values with taint metadata. Any result derived from a secret variable or secret response field is redacted when formatted. Taint is conservative: concatenation, selection, object construction, and array projection preserve taint.


---

# 5. Templates and Variables

## 5.1 Namespaces and precedence

Variable lookup order during a request:

1. captures from completed earlier steps;
2. CLI `--var` and `--secret` values;
3. selected environment profile;
4. inline `toml mdok vars` blocks;
5. project defaults;
6. built-in read-only values.

Duplicate definitions at the same level are errors. Environment variables from the process are not imported unless explicitly mapped in `mdok.toml` or passed with `--allow-env NAME`.

## 5.2 Template grammar

```ebnf
template       = "{{", wsp*, path, *(wsp*, "|", wsp*, filter), wsp*, "}}" ;
path           = identifier, *(".", identifier | "[", index, "]") ;
filter         = "string" | "raw" | "json" | "url" | "header" | "base64" ;
```

Templates are parsed into the Bash word AST. They are not implemented by global string replacement.

## 5.3 Filters

| Filter | Meaning |
|---|---|
| `string` | Scalar to UTF-8 string. Default. Objects/arrays are type errors. |
| `raw` | Exact scalar string with no additional encoding; still one argv value. |
| `json` | Canonical JSON serialization suitable for JSON bodies. |
| `url` | RFC 3986 percent-encoding for a path/query component. |
| `header` | String with CR and LF forbidden; prevents header injection. |
| `base64` | Standard Base64 encoding of string/bytes. |

`{{value}}` is equivalent to `{{value|string}}`. No filter causes shell evaluation.

## 5.4 Quoting model

Quotes belong to the shell source and are removed while building argv. Template values become data inside the resulting argument. A value containing quote characters, spaces, semicolons, `$()`, or newlines cannot create a new argument or command.

Example:

```curl
curl --header "X-Display: {{display_name|header}}" "{{base_url}}/me"
```

If `display_name` is `W \"Admin\"`, the header argument remains one argv element. If it contains CR or LF, the header filter fails before execution.

## 5.5 Secret declarations

Project configuration may declare secret sources:

```toml
[env.staging.secrets]
api_token = { from_env = "STAGING_API_TOKEN" }
```

CLI:

```bash
mdok test api.md --secret api_token=@prompt
mdok test api.md --secret api_token=@file:token.txt
```

Interactive prompts are prohibited in CI mode.

## 5.6 Captured variable lifecycle

- Captures are document-run scoped.
- They are cleared between retries of the whole document.
- A failed source step publishes no captures.
- Capture objects are immutable after publication.
- Secrets can be marked by project policy paths, for example `body.access_token`.


---

# 6. CLI and Configuration

## 6.1 Commands

```text
mdok [test] <PATH...>        Parse, plan, execute, and check.
mdok lint <PATH...>          Parse and statically validate without network access.
mdok plan <PATH...>          Print the normalized execution plan; redact secrets.
mdok list <PATH...>          List documents, steps, checks, and captures.
mdok version                 Print MDOK, curl, libcurl, TLS, and feature versions.
```

`mdok file.md` is an alias for `mdok test file.md`.

## 6.2 Common options

```text
--config <path>              Project configuration; default search is mdok.toml upward.
--env <name>                 Select environment profile.
--var key=value              Set a non-secret variable; repeatable.
--secret key=value           Set a secret; repeatable.
--allow-host <pattern>       Add an allowed destination host.
--deny-host <pattern>        Deny a host even if otherwise allowed.
--jobs <n>                   Parallel documents; steps remain ordered.
--fail-fast                  Stop after first failed assertion/step.
--timeout <duration>         Global per-transfer ceiling.
--max-body <bytes>           Captured body limit.
--json                       Emit one JSON report to stdout.
--json-lines                 Stream event records.
--junit <path>               Write JUnit XML.
--report <path>              Write JSON report atomically.
--no-color                   Disable ANSI output.
--offline                    Deny all network execution; useful with lint/plan.
--seed <u64>                 Deterministic seed for future generators.
```

## 6.3 Exit codes

| Code | Meaning |
|---:|---|
| 0 | All selected documents passed. |
| 1 | One or more checks or transfers failed. |
| 2 | Parse, configuration, or planning error. |
| 3 | Permission/policy denial. |
| 4 | Internal error or invariant failure. |
| 130 | Interrupted by user. |

## 6.4 Configuration

```toml
language = "1"
curl_compat = "8.21"

[execution]
jobs = 4
fail_fast = false
max_body_bytes = 8388608
memory_body_threshold_bytes = 262144
connect_timeout = "5s"
total_timeout = "30s"
allowed_schemes = ["http", "https"]

[policy]
allowed_hosts = ["127.0.0.1", "localhost", "api.example.com"]
allowed_read_paths = ["tests/fixtures/**"]
allowed_write_paths = []
allow_insecure_tls = false
allow_proxy = false
allow_unix_sockets = false

[vars]
region = "ap-southeast-1"

[env.local.vars]
base_url = "http://127.0.0.1:9800"

[env.staging.vars]
base_url = "https://staging.example.com"

[env.staging.secrets]
api_token = { from_env = "STAGING_API_TOKEN" }
```

## 6.5 Discovery

Directory input recursively discovers `.md` and `.mdok.md` files, honoring `.gitignore` by default. Hidden directories and `target`, `.git`, `node_modules`, and `vendor` are skipped unless explicitly selected.

## 6.6 Output contract

Human output is concise by default. `--verbose` shows request metadata with secrets redacted. JSON output follows `specs/report.schema.json`. Event ordering is deterministic for sequential runs and includes stable sequence numbers for parallel runs.


---

# 7. Architecture

## 7.1 Component diagram

```text
mdok-cli
  -> mdok-core
     -> mdok-markdown (Comrak AST + source map)
     -> mdok-template (template parser + typed values + taint)
     -> mdok-shell (Tree-sitter Bash restriction + argv builder)
     -> mdok-curl (safe Rust wrapper)
        -> mdok-curl-sys (FFI declarations/build)
           -> native/mdok_curl_bridge.c
              -> patched curl tool parser
              -> libcurl multi
     -> mdok-jmespath (compile/evaluate)
     -> mdok-runtime (plan/scheduler/state/limits)
     -> mdok-command (trusted direct-argv process groups/limits)
     -> mdok-report (human/JSON/JUnit/events)
```

## 7.2 Planning pipeline

1. Read UTF-8 source with a configurable file-size limit.
2. Parse Markdown with Comrak into an AST.
3. Walk executable code-block nodes and preserve source spans and heading paths.
4. Parse fence metadata with the info-string parser.
5. Parse TOML variable blocks.
6. Parse each curl block with Tree-sitter Bash; validate restricted AST.
7. Parse each `exec` block as direct argv; validate its profile and argument policy.
8. Parse templates inside accepted word nodes into a typed word plan.
9. Build placeholder-safe argv and call the C curl parser in parse-only mode for curl sources.
10. Apply MDOK curl or trusted-command policy.
11. Compile JMESPath checks and captures.
12. Validate references, uniqueness, order, and variable availability.
13. Produce an immutable `DocumentPlan`.

No network operation occurs before planning succeeds for the whole selected document.

## 7.3 Execution pipeline

1. Create an `ExecutionSession` with cancellation token, limits, cookie/share state, and libcurl multi handle.
2. Resolve templates for the next step into exact argv strings.
3. For curl sources, re-parse argv through the C curl parser if values can affect parser semantics; otherwise bind values into a prevalidated plan. Version 1 chooses re-parse for correctness.
4. For `exec` sources, validate the named profile and run the direct argv in a bounded process group/job object with a cleared environment.
5. Enforce resolved URL, path, proxy, and TLS policy for curl sources.
6. Execute curl with libcurl multi and callbacks, or the approved external command.
7. Stream curl bodies into memory until threshold, then spool to a private temporary file; keep external stdout/stderr under a combined byte budget.
8. Construct the typed transfer or external execution result.
9. Compile/parse body or command-output representation under limits.
10. Evaluate all checks.
11. If checks pass, evaluate and publish captures.
12. Emit events and proceed.

## 7.4 Data types

Key immutable types:

```rust
pub struct SourceSpan { pub byte_start: usize, pub byte_end: usize, pub line: u32, pub column: u32 }

pub struct DocumentPlan {
    pub path: PathBuf,
    pub language_version: LanguageVersion,
    pub variables: ValueMap,
    pub steps: Vec<StepPlan>,
}

pub struct StepPlan {
    pub name: StepName,
    pub heading_path: Vec<String>,
    pub source: StepSource,
    pub checks: Vec<CheckPlan>,
    pub captures: Vec<CapturePlan>,
    pub span: SourceSpan,
}

pub enum StepSource {
    Curl(CurlSourcePlan),
    Exec(ExecSourcePlan),
}

pub struct ExecSourcePlan {
    pub source: String,
    pub span: SourceSpan,
}

pub struct TransferResult {
    pub status: Option<u16>,
    pub method: String,
    pub effective_url: String,
    pub headers: HeaderMapVec,
    pub body: BodyArtifact,
    pub timings: Timings,
    pub transfer: TransferMetrics,
    pub redirects: Vec<RedirectHop>,
    pub error: Option<TransferError>,
}
```

## 7.5 Concurrency

- Steps in one document are sequential in version 1 because captures create implicit dependencies.
- Independent documents may run concurrently with `--jobs`.
- One `CURLM` multi handle is owned by each execution worker/session.
- Connection/DNS/TLS-session sharing is allowed only through libcurl-supported mechanisms and synchronized handles.
- Event records contain document index, step index, and monotonically increasing sequence IDs.

## 7.6 Cancellation

Rust owns a cancellation token. The C bridge checks it from `CURLOPT_XFERINFOFUNCTION`; returning non-zero aborts the transfer. Multi polling uses bounded waits so Ctrl-C latency remains low.

## 7.7 Error boundaries

Every layer returns a typed error with a stable code and optional source span. Internal errors preserve a causal chain for debug reports but normal output avoids stack traces and secrets.


---

# 8. C/Rust Boundary and curl Integration

## 8.1 Ownership rule

Rust never accesses curl tool internal structures. C owns all curl tool parser objects, libcurl handles, linked lists, MIME objects, and error buffers. Rust receives opaque handles and copied, length-delimited data.

## 8.2 Stable bridge API

See `repo-skeleton/native/include/mdok_curl.h`. The API is versioned independently of curl internals.

Core operations:

```c
mdok_curl_status mdok_curl_global_init(const mdok_curl_global_options *options);
void mdok_curl_global_cleanup(void);

mdok_curl_status mdok_curl_parse(
    const mdok_curl_argv *argv,
    const mdok_curl_policy *policy,
    mdok_curl_plan **out_plan,
    mdok_curl_error *out_error);

mdok_curl_status mdok_curl_execute(
    mdok_curl_session *session,
    const mdok_curl_plan *plan,
    const mdok_curl_callbacks *callbacks,
    void *userdata,
    mdok_curl_result *out_result,
    mdok_curl_error *out_error);

void mdok_curl_plan_free(mdok_curl_plan *plan);
```

## 8.3 Strings and buffers

- All strings are UTF-8 unless explicitly byte buffers.
- Every string/buffer crossing FFI has pointer plus length; no unbounded `strlen` on Rust-owned data.
- C copies data it retains after a call.
- Rust callback data is valid only for the callback duration unless copied.
- Null and empty are distinct where required.

## 8.4 Panic and failure safety

- Rust callbacks use `catch_unwind`; panic becomes cancellation/internal error.
- C does not call `exit`, abort the process, or permit curl tool fatal paths to escape.
- The curl tool patch replaces direct process termination with error returns.
- All cleanup paths are idempotent and sanitizer-tested.

## 8.5 Thread model

`curl_global_init` is called once before workers. A session and its multi handle are confined to one worker thread. Immutable parsed plans may be sent across threads only if the C bridge explicitly guarantees it; version 1 keeps plan and session on the same worker to reduce risk.

## 8.6 Build strategy

1. `scripts/fetch-curl.sh` downloads and verifies the pinned curl release.
2. Patches under `vendor/patches/curl/` expose a static curl-tool parser library and remove process-global terminal assumptions.
3. CMake builds libcurl, the parser library, and `mdok_curl_bridge` with hidden symbol visibility.
4. `mdok-curl-sys/build.rs` invokes CMake and links the static bridge.
5. Release builds default to bundled curl for deterministic features. A `system-curl` feature is development-only until compatibility is proven.

## 8.7 Required C tests

- parser allocation failure injection;
- malformed argv and missing option argument;
- repeated parse/free loops;
- execute/cancel/free races under ThreadSanitizer where supported;
- header/body callback short writes;
- body spool failure;
- libcurl error buffer handling;
- curl tool upgrade differential tests;
- AddressSanitizer, UndefinedBehaviorSanitizer, and leak checks.

## 8.8 Curl upgrade process

- Change one pinned version constant.
- Verify source archive checksum and signature where available.
- Reapply patch series with no fuzzy hunks.
- Regenerate option policy.
- Compile all supported targets.
- Run curl upstream tests applicable to the build.
- Run MDOK parser differential corpus.
- Review added/changed options and update policy explicitly.


---

# 9. Repository Structure

```text
mdok/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── mdok.toml
├── LICENSE
├── THIRD_PARTY.md
├── README.md
├── crates/
│   ├── mdok-cli/             # clap entry point, discovery, process exit
│   ├── mdok-core/            # public facade and shared value/error types
│   ├── mdok-markdown/        # Comrak AST extraction and source maps
│   ├── mdok-template/        # template grammar, filters, values, taint
│   ├── mdok-shell/           # Tree-sitter Bash restriction and argv builder
│   ├── mdok-curl-sys/        # unsafe FFI declarations and native build
│   ├── mdok-curl/            # safe Rust wrapper around mdok-curl-sys
│   ├── mdok-jmespath/        # compile/evaluate and typed diagnostics
│   ├── mdok-runtime/         # planning, execution state, scheduler, limits
│   ├── mdok-report/          # event stream, human, JSON, JUnit
│   └── mdok-test-server/     # deterministic HTTP/HTTPS fixture service
├── native/
│   ├── CMakeLists.txt
│   ├── include/mdok_curl.h
│   └── src/
│       ├── mdok_curl_global.c
│       ├── mdok_curl_parse.c
│       ├── mdok_curl_plan.c
│       ├── mdok_curl_execute.c
│       ├── mdok_curl_callbacks.c
│       ├── mdok_curl_policy.c
│       └── mdok_curl_error.c
├── vendor/
│   ├── curl/                 # populated by script or submodule
│   ├── curl.version
│   ├── curl.sha256
│   └── patches/curl/*.patch
├── specs/
│   ├── language.ebnf
│   ├── response.schema.json
│   ├── report.schema.json
│   ├── corpus-manifest.schema.json
│   ├── error-codes.md
│   └── curl-option-policy.csv
├── tests/
│   ├── corpus/index.jsonl
│   ├── corpus/<category>/*.md
│   ├── fixtures/files/*
│   ├── fixtures/tls/*
│   ├── integration/
│   ├── differential/
│   └── fuzz/
├── fuzz/
│   ├── markdown/
│   ├── fence_info/
│   ├── template/
│   ├── shell/
│   └── ffi/
├── benches/
│   ├── parse.rs
│   ├── plan.rs
│   ├── jmespath.rs
│   └── transfer.rs
├── scripts/
│   ├── fetch-curl.sh
│   ├── sync-curl-options.py
│   ├── generate-corpus.py
│   ├── validate-corpus.py
│   └── release.sh
└── .github/workflows/
    ├── ci.yml
    ├── sanitizers.yml
    ├── fuzz-smoke.yml
    └── release.yml
```

## 9.1 Dependency direction

Lower-level crates cannot depend on the CLI or runtime. `mdok-core` contains only shared types and facades, not a dependency grab bag. `mdok-curl-sys` is the only crate with unsafe FFI declarations. All other unsafe code requires a documented safety invariant and local tests.

## 9.2 Public API

The first stable product surface is the CLI and JSON report schema. Rust library APIs remain semver-unstable until the execution model has shipped and been used in external integrations.


---

# 10. Security and Sandboxing

## 10.1 Threat model

An MDOK document may be untrusted and can attempt SSRF, local file disclosure, credential leakage, denial of service, unsafe redirects, proxy abuse, DNS rebinding, path traversal, or parser/resource exhaustion.

## 10.2 Default posture

- No shell execution.
- External commands are opt-in, direct-argv only, and resolved through explicitly configured canonical executable profiles; ambient `PATH` lookup is never used.
- External command children receive a cleared environment containing only configured mappings, with secret mappings tracked for taint and redaction.
- External commands run in bounded process groups or Windows Job Objects with timeout, argument, and combined stdout/stderr limits.
- HTTP/HTTPS only.
- Implicit curl config disabled.
- Interactive credential prompts disabled.
- Local file reads denied unless inside allowlisted project paths.
- File writes denied in version 1 except private temporary spooling and explicit report paths.
- Proxy use denied unless configured.
- Unix sockets denied unless configured.
- `--insecure` denied unless policy allows it.
- Redirects are rechecked against host/scheme policy on every hop.
- Link-local, loopback, private, and metadata addresses can be independently denied.
- DNS answers are checked at connect time, not only URL parse time.

## 10.3 SSRF policy

Host policy supports exact hosts and anchored wildcard suffixes. Resolved IP policy supports CIDRs. Both the hostname and every resolved/connected address must pass. Redirect and `--resolve`/`--connect-to` targets are checked. Cloud metadata ranges are denied by default outside explicit local-test mode.

## 10.4 Filesystem policy

All curl options that reference files are normalized relative to the document/project root, canonicalized without following unsafe symlink escapes, and checked against read/write glob policies. `@-`, `/dev/*`, device paths, named pipes, and Windows device namespaces are denied by default.

## 10.5 Secrets

- CLI secret values are never included in process titles beyond unavoidable user invocation; `@file` and environment mapping are preferred.
- Arguments passed to the in-process C parser do not create a child process.
- Reports redact exact secret values and derived tainted values.
- Request headers commonly carrying credentials are redacted by name.
- Debug traces require explicit opt-in and remain redacted by default.
- Temporary files use owner-only permissions and are unlinked promptly.

## 10.6 Resource limits

- Maximum source bytes per document.
- Maximum AST nodes and executable blocks.
- Maximum argv elements and bytes.
- Maximum template count and expansion bytes.
- Maximum request body and upload bytes.
- Maximum response headers, individual header size, body bytes, redirects, retries, and total time.
- Maximum JSON nesting and JMESPath output size.
- Maximum concurrent documents and open files.
- Maximum external command arguments, total argv bytes, combined output bytes, and execution time.

## 10.7 Supply-chain controls

- Pinned curl source checksum and provenance.
- `cargo vet` or equivalent dependency review policy.
- Locked dependencies for release.
- SBOM generation.
- Reproducible-build checks where practical.
- Signed release checksums.


---

# 11. Errors and Diagnostics

## 11.1 Required fields

Every diagnostic contains:

- stable code;
- severity;
- concise title;
- explanatory message;
- source file and smallest useful span;
- document/step/check identifiers when available;
- redacted observed values;
- cause chain for machine output;
- optional repair hint that does not claim certainty.

External command diagnostics use `MDOK-E306` through `MDOK-E312` for policy,
argv, start/reap, exit, timeout, resource-limit, and environment/working-
directory failures. Command stdout/stderr may be available transiently in the
step context for checks, but is not copied into durable reports; secret-tainted
output is redacted and cannot be captured.

## 11.2 Example

```text
error[MDOK-E201]: shell operator is not allowed in a curl block

  tests/auth.md:12:46

12 │ curl "{{base_url}}/me" | jq '.user'
   │                          ^ shell pipelines would execute another program

Step: get_me
Allowed: one simple command whose first word is `curl`
Use a JMESPath check or capture block instead of piping to jq.
```

## 11.3 Agent JSON

```json
{
  "schema_version": "1",
  "code": "MDOK-E502",
  "kind": "assertion_failed",
  "file": "tests/auth.md",
  "step": "login",
  "span": {"byte_start": 413, "byte_end": 428, "line": 19, "column": 1},
  "expression": "status == `200`",
  "result": false,
  "observed": {"status": 401},
  "redactions": ["response.body.access_token"],
  "hint": "Confirm credentials or update the expected status."
}
```

## 11.4 Stability

Error codes and JSON field meanings are compatibility surfaces. Human wording may improve without a major version. Do not make automation depend on full human messages.

See `specs/error-codes.md` for the registry.


---

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


---

# 13. Performance and Memory

## 13.1 Targets

Measured on a current developer-class laptop with release builds:

- Cold `mdok version`: p50 < 50 ms.
- Parse and plan a 10 KB document with 10 steps: p50 < 2 ms.
- Parse and plan 1,000 2 KB documents: < 1 second with parallel discovery.
- Added per-transfer overhead excluding network: p50 < 0.5 ms.
- JMESPath compile: cached per expression; evaluation p50 < 100 microseconds for 10 KB JSON.
- Resident memory for 1,000 planned small documents: < 100 MB.
- Response body memory bounded by `memory_body_threshold_bytes` plus fixed parsing overhead.

## 13.2 Body handling

Body callbacks append to an in-memory buffer until the threshold. Larger bodies spool to a private temporary file. JSON parsing may use memory mapping or a bounded read; version 1 may refuse JMESPath body evaluation above `max_json_body_bytes` rather than allocate unbounded memory.

## 13.3 Connection reuse

Reuse libcurl easy handles where safe and retain the multi handle for a document/session. Avoid rebuilding DNS caches and TLS sessions for sequential calls to the same origin. Tests must prove state reset between steps so headers, methods, bodies, and authentication do not leak.

## 13.4 Benchmarks

Required Criterion groups:

- `markdown_extract/{size,blocks}`;
- `shell_parse/{argv_bytes,templates}`;
- `curl_parse/{options}`;
- `jmespath_compile/{complexity}`;
- `jmespath_eval/{json_size,expression}`;
- `body_capture/{memory,spill,binary}`;
- `report/{events}`;
- `end_to_end/{steps,keepalive}`.

Track allocations with platform tooling and add a regression budget in CI rather than only wall-clock thresholds.


---

# 14. Portability and Build

## 14.1 Supported targets

Tier 1 target set:

- macOS arm64 and x86_64;
- Linux x86_64 and aarch64 using glibc;
- Windows x86_64 MSVC.

Musl static builds are Tier 2 because static TLS/libcurl dependency composition is more complex. FreeBSD and other Unix targets are community-supported initially.

## 14.2 Bundled curl

Release artifacts bundle a known curl/libcurl build so behavior does not depend on the host's curl executable. TLS backend choices should match platform expectations where possible:

- macOS: Secure Transport/SecTrust-compatible build or OpenSSL/rustls after validation;
- Windows: Schannel by default;
- Linux: OpenSSL or rustls, chosen and documented per artifact.

Feature matrices are printed by `mdok version --json`.

## 14.3 Local development prerequisites

- Rust stable toolchain pinned by `rust-toolchain.toml`;
- C11 compiler;
- CMake and Ninja;
- Python 3 for generation/test scripts;
- platform TLS/build dependencies when not using fully vendored dependencies.

## 14.4 Reproducibility

Release builds use Cargo lockfiles, pinned curl source and checksum, pinned patch series, explicit CMake options, and recorded compiler versions. Build metadata is embedded in `mdok version` without making binaries nondeterministic where reproducibility is enabled.


---

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


---

# 16. Decisions and Open Questions

## Final decisions

- Rust is the product/runtime language; C is limited to curl integration.
- Comrak parses Markdown.
- Tree-sitter Bash parses curl fence shell structure.
- curl's actual tool parser interprets curl options.
- libcurl performs transfers.
- Standard JMESPath handles checks and captures.
- TOML handles project and inline variable tables.
- Version 1 executes one transfer per curl fence and no arbitrary shell.
- Version 1 is sequential within one document.

## Open questions requiring implementation spikes

1. How small can the curl-tool patch remain while exposing a stable parse plan?
2. Should the bridge execute from curl's internal `OperationConfig` directly, or convert to an MDOK-owned plan before execution?
3. Which TLS backend provides the best release portability on macOS without surprising trust-store behavior?
4. How should huge JSON bodies be queried without violating memory bounds?
5. Is conservative secret taint practical through the selected Rust JMESPath implementation, or should redaction use value fingerprinting plus path policy in version 1?
6. Should `ws`/`wss` be version 1 or a later feature?
7. Should explicitly referenced curl config files be supported in version 1 or rejected until policy handling is mature?

None of these questions changes the user-facing core language. Phase 0 should resolve the curl bridge questions before broad implementation.


---

# 17. Deterministic Fixture Server

## 17.1 Startup contract

`mdok-test-server --listen 127.0.0.1:0 --tls-listen 127.0.0.1:0 --json-ready` prints exactly one readiness JSON object:

```json
{"http_base_url":"http://127.0.0.1:43123","https_base_url":"https://127.0.0.1:43124","proxy_url":"http://127.0.0.1:43125","ca_file":"/tmp/mdok-ca.pem"}
```

It then writes logs to stderr in JSON Lines. No public network is required.

## 17.2 Endpoints

| Endpoint | Behavior |
|---|---|
| `/health` | `200 {"ok":true}`. |
| `/echo` | Returns method, path, query, headers, cookies, and parsed/raw body. |
| `/status/{code}` | Returns selected status and deterministic JSON. |
| `/json/{case}` | Nested arrays/objects/nulls/numbers/unicode for JMESPath cases. |
| `/headers` | Duplicate, mixed-case, empty, long, and folded-invalid test variants. |
| `/auth/basic` | Validates fixed basic credentials. |
| `/auth/bearer` | Validates fixed bearer token. |
| `/auth/login` | Returns token/user object for workflow tests. |
| `/users/{id}` | CRUD-like deterministic user endpoints. |
| `/cookies/set` | Sets one or more cookies. |
| `/cookies/echo` | Returns received cookies. |
| `/redirect/{n}` | Redirect chain terminating at `/echo`; can change host/scheme for policy tests. |
| `/delay/{ms}` | Delays before headers. |
| `/stream/{chunks}/{delay_ms}` | Chunked response with deterministic chunks. |
| `/gzip` | Gzip-encoded JSON. |
| `/binary/{size}` | Deterministic non-UTF-8 bytes. |
| `/upload` | Returns size and SHA-256 of uploaded bytes. |
| `/multipart` | Returns parsed fields/files and hashes. |
| `/close/early` | Closes mid-response. |
| `/retry/{failures}` | Fails a deterministic number of times per test key, then succeeds. |
| `/large/{size}` | Bounded generated response for limit/spool tests. |

## 17.3 TLS

Tests use a generated local CA and leaf certificate with IP/DNS SANs. Keys are test-only. The harness provides the CA path; tests must not require `--insecure` except explicit policy cases.

## 17.4 Determinism

The server must not include wall-clock timestamps or random IDs unless a test supplies a seed/key. Per-test mutable state is namespaced by a unique header set by the harness.


---

# 18. Dependencies and Licensing

## 18.1 Planned primary dependencies

- Comrak for CommonMark/GFM parsing and AST/source positions.
- Tree-sitter and Tree-sitter Bash for restricted shell syntax parsing.
- A Rust JMESPath implementation plus the upstream compliance suite.
- TOML/Serde for configuration and typed values.
- Clap for the CLI.
- Tokio or an equivalent runtime only if it materially simplifies multi-handle polling and fixture-server implementation; avoid async complexity in parser crates.
- curl source and libcurl under curl's permissive license.

Exact dependency versions are pinned at implementation start and updated through review. The PRD does not assume that a crate's API is stable merely because its semver is stable.

## 18.2 License recommendation

Apache-2.0 OR MIT is recommended for MDOK. Curl notices and the curl license must be included in binary/source distributions. Generated SBOM and `THIRD_PARTY.md` identify bundled components and enabled features.

## 18.3 Dependency policy

- Prefer mature parsers and standards implementations over custom regex/line parsers.
- Minimize unsafe and transitive build-time execution.
- No GPL dependency in the shipped binary unless the project's license strategy explicitly accepts it.
- Audit parsing, TLS, serialization, and FFI dependencies more strictly than convenience libraries.


---

# 19. Release and Supply Chain

## 19.1 Artifacts

- macOS universal or separate arm64/x86_64 archives;
- Linux x86_64/aarch64 archives;
- Windows x86_64 ZIP;
- SHA-256 checksums;
- SBOM in SPDX or CycloneDX;
- build provenance/attestation;
- shell installer only after direct archive installation is stable.

## 19.2 Release checks

- Clean checkout build.
- Pinned curl checksum and patch verification.
- Full corpus and generated option-policy tests.
- Sanitizers and fuzz smoke.
- Benchmark comparison.
- `mdok version --json` feature snapshot.
- License/notice verification.
- Malware/secret scan of artifacts.
- Install-and-run smoke test in clean VMs/containers.

## 19.3 Compatibility promises

- Language version changes are explicit.
- JSON report schema follows additive evolution within a major schema version.
- Error codes are not reassigned.
- Curl compatibility version is printed and recorded in reports.
- A newer curl option is never silently accepted before classification.


---

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

## F.1 Trusted direct-command adapter

- [x] Recognize `exec mdok name=...` fences as one direct argv command.
- [x] Require the first argv token to select an explicitly configured command profile.
- [x] Resolve profile programs to canonical absolute executable paths without ambient `PATH` lookup.
- [x] Reject shell interpreters, shell operators, empty/NUL arguments, and oversized argv.
- [x] Clear inherited environment and pass only declared fixed or secret mappings.
- [x] Enforce timeout, combined stdout/stderr, process-group, and descendant cleanup limits.
- [x] Expose bounded stdout/stderr, parsed JSON stdout, exit metadata, flags, and timing to checks.
- [x] Reject secret-tainted output captures and omit command output from durable reports.
- [x] Keep deterministic command fixtures and durable Markdown examples under `tests/agent-commands/`.

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


---

# 21. Parser and Runtime Algorithms

This document defines implementation algorithms tightly enough that separate C and Rust contributors should produce compatible behavior.

## 21.1 Executable-block extraction

```text
parse_document(source):
  reject invalid UTF-8 or source over limit
  ast = comrak.parse(source, configured_options)
  heading_stack = []
  blocks = []
  walk ast in source order:
    when heading:
      update heading_stack by heading level
    when fenced code block:
      info = parse_info_string(node.info, node.info_span)
      if info does not contain mdok: continue
      block = classify(language, info.attributes)
      attach original content span and heading_stack copy
      blocks.push(block)
  validate document-level names/references/order
  return blocks
```

Do not recover executable fences from malformed Markdown by scanning raw backticks. The Markdown AST is authoritative.

## 21.2 Template masking and Bash parsing

Templates must not be confused with Bash brace syntax.

```text
parse_curl_source(source):
  templates = template_parser.find_all(source)
  reject malformed/unclosed template syntax
  masked, source_map = replace each complete template span with one inert word token
                       that cannot create quotes, operators, expansions, or whitespace
  bash_tree = tree_sitter_bash.parse(masked)
  reject syntax errors
  require root -> one simple command only
  reject every forbidden node type
  for each accepted word node:
    map its masked span back to original spans
    parse quote/escape segments from the original word
    splice template AST nodes into those segments
  require evaluated first argv word to equal literal `curl`
  return TemplateAwareArgvPlan
```

The inert token may differ in byte length, but the source map must translate every Bash node span back to the original source. A simpler same-length mask is acceptable only if it is proven valid for arbitrary template length and Unicode byte spans.

## 21.3 Argument evaluation

```text
evaluate_argv(plan, variables):
  argv = []
  for argument in plan.arguments:
    output = byte/string builder with expansion limit
    for segment in argument.segments:
      literal -> append quote-removed/escape-decoded literal
      template -> lookup typed value; apply filter; append resulting data
    argv.push(output)
  assert argv count equals plan argument count
  assert argv[0] == "curl"
  return argv
```

Inserted values never return to the Bash parser. A value containing spaces, quotes, semicolons, newlines, `$()`, or operators stays inside one argv element. The `header` filter rejects CR/LF before C receives argv.

## 21.4 C parser bridge

```text
mdok_curl_parse(argv, policy):
  prepend/inject deterministic curl-tool settings such as -q
  initialize GlobalConfig and first OperationConfig using patched curl tool helpers
  call the real curl parse_args/getparameter path
  convert parser failures to mdok_curl_error without printing/exiting
  enumerate all operations/URLs generated by curl parser
  require exactly one logical transfer
  inspect options and feature requirements
  apply option classification and static protocol/file policy
  freeze an opaque plan or convert to an MDOK-owned C plan
  free transient parser state
  return plan
```

The bridge must not infer options by re-parsing strings. curl's parser output is authoritative.

## 21.5 Runtime policy and execution

```text
execute_step(step, state, session):
  argv = evaluate_argv(step.argv_plan, state.variables)
  c_plan = mdok_curl_parse(argv, policy)
  resolved = inspect c_plan resolved URL/files/proxy/connect overrides
  enforce_runtime_policy(resolved)
  sink = BodySink(memory_threshold, max_body)
  result_meta = mdok_curl_execute(session, c_plan, callbacks(sink, headers, cancellation))
  transfer_result = construct_result(result_meta, headers, sink)
  parse_body_under_limits(transfer_result)
  eval_context = build_context(transfer_result, state.variables, state.step_summaries)
  failures = evaluate_all_checks(step.checks, eval_context)
  if failures: return failed result without publishing captures
  captured = merge capture-object results with collision checks
  taint captured values from source paths/secrets
  publish captures atomically
  return passed result
```

## 21.6 Body sink state machine

```text
Memory(bytes <= threshold)
  on append crossing threshold -> create private temp, write existing bytes, transition File
File(path, length <= max_body)
  on append -> write fully or fail
Any
  on length > max_body -> mark limit error, abort callback/transfer
  on cancellation -> abort and clean
  on finish -> expose immutable BodyArtifact
```

All callback writes must handle partial filesystem writes and integer overflow. Temporary files are created with owner-only permissions and never use attacker-controlled names.

## 21.7 Header parser state machine

- Receive raw header callback bytes exactly as libcurl emits them.
- Split complete CRLF/LF lines while tolerating callback chunk boundaries.
- Recognize status lines, interim responses, redirect responses, and final response.
- Reject or preserve malformed header lines according to libcurl outcome; do not invent normalized values.
- Lowercase valid field names for the evaluation map.
- Trim optional whitespace around field values while preserving internal bytes converted to UTF-8 according to the documented policy.
- Append duplicate values to arrays.
- Enforce total header bytes, line count, field-name length, and field-value length.

## 21.8 Check evaluation

```text
for check in source_order:
  result = compiled_expression.search(context)
  parse/runtime error -> typed diagnostic
  boolean true -> pass
  boolean false -> assertion diagnostic
  any other type -> result-type diagnostic
```

With normal mode, evaluate all checks after a completed transfer. With `--fail-fast`, stop after the first failure. A transfer error can still expose a partial structured result, but checks run only if the check policy explicitly permits error-object checks in a future version; version 1 treats transfer failure as step failure.

## 21.9 Capture publication

Evaluate every capture expression against the same immutable context. Validate all objects and keys first. Merge into a temporary map. Publish the entire map atomically only after all capture expressions succeed. This prevents partially available state.

## 21.10 Document scheduler

Version 1 uses a linear state machine per document:

```text
Planned -> Running(step 0) -> Running(step 1) -> ... -> Passed
                         \-> Failed
                         \-> Cancelled
```

Multiple documents may run on a bounded worker pool. One document never migrates between workers while its C/libcurl session is active.

## 21.11 Deterministic events

Every event contains:

- run ID;
- document ordinal and normalized path;
- step ordinal/name;
- check/capture ordinal where applicable;
- worker-local timestamp and duration;
- globally assigned monotonically increasing sequence number at report aggregation.

Final JSON arrays are sorted by document input order and source order, not completion time.
