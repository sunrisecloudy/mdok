# MDOK

MDOK turns Markdown into executable API workflows. Write a readable API
example once, then lint it, plan it safely, run it, and keep it as a verified
contract for agents, humans, and CI.

## Try it

Build MDOK and validate the included example without making a network request:

```sh
cargo build --release
cargo run -p mdok-cli -- lint mdok-prd/examples/auth-flow.md
cargo run -p mdok-cli -- plan mdok-prd/examples/auth-flow.md --offline
```

Run the complete deterministic local E2E suite with its fixture server:

```sh
make e2e-md
```

## Your first workflow

Start with [`mdok-prd/examples/auth-flow.md`](mdok-prd/examples/auth-flow.md),
or create your own Markdown file with a request, variables, and checks. For a
live run, provide the API host explicitly:

```sh
cargo run -p mdok-cli -- test api.md \
  --var base_url=https://api.example.test \
  --allow-host api.example.test
```

Keep credentials out of the file. Pass them with `--secret` or load a dotenv
file only when you explicitly name it with `--env-file`.

## Where to go next

- [Getting started](docs/GETTING_STARTED.md) — variables, dotenv files,
  Postman import, reports, and local fixtures.
- [Agent workflow](docs/AGENT_WORKFLOW.md) — intent → example → plan/debug →
  approval → verified workflow → CI.
- [MCP server](docs/MCP.md) — tool schemas, policy, and agent integration.
- [Markdown E2E suite](tests/e2e/INDEX.md) — one focused feature per file plus
  a combined workflow.
- [Developer and maintainer guide](docs/DEVELOPMENT.md) — repository layout,
  command tests, performance, release signing, and TLS validation.
- [Documentation index](docs/README.md) — all product and engineering docs.

For coding agents, the reusable instructions are in
[`skills/mdok/SKILL.md`](skills/mdok/SKILL.md).
