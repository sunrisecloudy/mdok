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
