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

MDOK first parses and masks complete `{{...}}` template spans with inert source-mapped tokens, then parses the masked command with Tree-sitter Bash. This prevents templates from being mistaken for Bash brace syntax. Version 1 accepts exactly:

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
