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
     -> mdok-command (trusted direct argv profiles and bounded process groups)
     -> mdok-report (human/JSON/JUnit/events)
```

## 7.2 Planning pipeline

1. Read UTF-8 source with a configurable file-size limit.
2. Parse Markdown with Comrak into an AST.
3. Walk executable code-block nodes and preserve source spans and heading paths.
4. Parse fence metadata with the info-string parser.
5. Parse TOML variable blocks.
6. Parse template spans, mask them with inert source-mapped tokens, and parse each curl block with Tree-sitter Bash; tokenize each `exec` block without shell evaluation.
7. Validate the restricted Bash AST or direct-argv grammar and reconstruct typed word segments from the original source plus template AST.
8. Build placeholder-safe argv and call the C curl parser in parse-only mode for curl steps.
9. Apply MDOK curl option/scheme/filesystem policy or trusted command-profile policy.
10. Compile JMESPath checks and captures.
11. Validate references, uniqueness, order, and variable availability.
12. Produce an immutable typed `DocumentPlan` with `StepSource::Curl` or `StepSource::Exec`.

No network operation occurs before planning succeeds for the whole selected document.

## 7.3 Execution pipeline

1. Create an `ExecutionSession` with cancellation token, limits, cookie/share state, and libcurl multi handle.
2. Resolve templates for the next step into exact argv strings.
3. Re-parse argv through the C curl parser if values can affect parser semantics; otherwise bind values into a prevalidated plan. Version 1 chooses re-parse for correctness.
4. Enforce resolved URL, path, proxy, and TLS policy.
5. Execute with libcurl multi and callbacks.
6. Stream body into memory until threshold, then spool to a private temporary file.
7. Construct the typed transfer result.
8. Compile/parse body representation under limits.
9. Evaluate all checks.
10. If checks pass, evaluate and publish captures.
11. Emit events and proceed.

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
