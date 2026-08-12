# Getting started with MDOK

This guide covers the first useful workflow after reading the root
[README](../README.md).

## Build and inspect an example

```sh
cargo build --release
cargo run -p mdok-cli -- lint mdok-prd/examples/auth-flow.md
cargo run -p mdok-cli -- list mdok-prd/examples/auth-flow.md
cargo run -p mdok-cli -- plan mdok-prd/examples/auth-flow.md --offline
```

`lint` validates the Markdown without network access. `list` shows the
requests, checks, and captures. `plan` renders the normalized, redacted plan
so you can review it before a live run.

## Run a workflow

Use an explicit host allowlist for a live API:

```sh
cargo run -p mdok-cli -- test api.md \
  --var base_url=https://api.example.test \
  --allow-host api.example.test \
  --json
```

The same document can produce JSON Lines or JUnit output for automation:

```sh
cargo run -p mdok-cli -- test api.md --json-lines
cargo run -p mdok-cli -- test api.md --junit target/mdok-api.xml
```

The bare form `mdok api.md` is an alias for `mdok test api.md`.

## Variables and dotenv files

Use `--var KEY=VALUE` for non-secret values and `--secret KEY=VALUE` for
credentials. Dotenv files are never discovered implicitly:

```sh
mdok plan api.md --env-file .env.local
mdok test api.md --env-file .env.local --env-file .env.private
```

Later dotenv files override earlier files, and explicit `--var`/`--secret`
assignments override dotenv values. MDOK parses assignments literally without
shell commands or variable expansion. Names such as `TOKEN`, `PASSWORD`,
`SECRET`, and `API_KEY` remain secret-tainted and are redacted.

## Postman migration

Convert a Postman Collection v2.1 into reviewable Markdown:

```sh
mdok import postman collection.json --out api.mdok.md
```

Review the generated Markdown and its import manifest before running it. The
importer fails closed when a collection contains behavior that cannot be
represented safely. See [POSTMAN_IMPORT.md](POSTMAN_IMPORT.md).

## Deterministic local testing

Run the repository's complete local fixture suite with:

```sh
make e2e-md
```

The suite starts and stops the loopback fixture server for you. Its focused
workflows and combined example are listed in
[`tests/e2e/INDEX.md`](../tests/e2e/INDEX.md).
