# MDOK

MDOK turns ordinary Markdown into executable API workflow tests. Request blocks use copied `curl` syntax, checks and captures use standard JMESPath, and the CLI produces human, JSON, JSON Lines, and JUnit reports.

## Quick start

```sh
cargo build --release
cargo run -p mdok-cli -- lint mdok-prd/examples/auth-flow.md
cargo run -p mdok-cli -- plan mdok-prd/examples/auth-flow.md --var base_url=http://127.0.0.1:9800
cargo run -p mdok-cli -- test mdok-prd/examples/auth-flow.md --var base_url=http://127.0.0.1:9800
```

The bare form `mdok file.md` is an alias for `mdok test file.md`. Use `mdok-test-server --listen 127.0.0.1:0 --json-ready` for deterministic local HTTP fixtures.

## Project files

- `mdok.toml` controls language, execution limits, variables, and network/filesystem policy.
- `mdok-prd/` contains the product contract and the 495-fixture corpus.
- `crates/` contains the parser, template, restricted shell, curl, runtime, reporting, CLI, and fixture-server components.

MDOK performs whole-document planning before a request is sent. Interpolated values are data inside one argv element; they are never reparsed as shell source. HTTP/HTTPS and loopback-safe local testing are the default execution surface.

