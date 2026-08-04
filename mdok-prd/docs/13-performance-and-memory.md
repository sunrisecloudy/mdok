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
