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
│   ├── mdok-command/         # trusted direct argv execution and process limits
│   ├── mdok-command-fixture/ # deterministic external command test binary
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
