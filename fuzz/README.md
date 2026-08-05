# MDOK fuzz targets

These targets use `cargo-fuzz` and the public MDOK APIs. They are deliberately
kept outside the main Cargo workspace because cargo-fuzz owns a separate fuzz
workspace and nightly/libFuzzer build configuration.

Targets:

- `markdown`: arbitrary bytes through Markdown fence-info parsing, document
  parsing, and planning;
- `shell_template`: arbitrary template and restricted-shell source through
  template parsing/rendering and curl argv parsing/evaluation;
- `curl_ffi`: bounded arbitrary argv slices through the opaque native curl
  parser and owned `Plan` destructor.

Run a bounded local smoke pass from the repository root:

```sh
scripts/run_fuzz_smoke.sh
```

When `cargo-fuzz` is unavailable, the runner executes a deterministic
byte/UTF-8 corpus test under `fuzz/tests/smoke.rs` so parser-boundary coverage
and panic checks remain available without installing another tool. The
libFuzzer targets are still preferred whenever `cargo-fuzz` is present.

Useful overrides are `MDOK_FUZZ_RUNS`, `MDOK_FUZZ_MAX_LEN`,
`MDOK_FUZZ_TIMEOUT`, and `MDOK_FUZZ_SANITIZER`.
