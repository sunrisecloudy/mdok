# MDOK MCP server

`mdok mcp serve` exposes MDOK as a [Model Context Protocol](https://modelcontextprotocol.io/)
server over stdin/stdout. It gives coding agents one safe control plane for the
full API-development loop: show an example, try or debug it, verify the
contract, and hand the same Markdown artifact to CI.

The Markdown file is the durable artifact. MCP tools inspect, plan, and execute
it; the agent's workspace tools save or update it. This separation keeps
authoring reviewable and prevents a tool call from silently replacing a checked
in workflow.

For the short intent-to-CI workflow, see
[AGENT_WORKFLOW.md](AGENT_WORKFLOW.md). This document focuses on MCP's tool
schemas, policy, and transport.

## Start the server

Build the CLI and point an MCP client at the executable:

```sh
cargo build --release -p mdok-cli
./target/release/mdok mcp serve
```

A generic MCP configuration is:

```json
{
  "mcpServers": {
    "mdok": {
      "command": "/absolute/path/to/mdok",
      "args": ["mcp", "serve"]
    }
  }
}
```

The server writes only MCP protocol messages to stdout. Child command output
is returned as structured JSON reports, with raw stderr omitted to avoid
accidentally exposing credentials.

## Tools

The server advertises schemas through `tools/list`; clients should discover the
schema rather than hard-code optional fields.

- `mdok_lint` validates one or more Markdown files without network execution.
- `mdok_plan` returns the normalized, redacted execution plan.
- `mdok_list` inventories requests, checks, captures, and dependencies.
- `mdok_test` executes the workflow and returns the normal MDOK JSON report.
- `mdok_probe` runs a bounded Postman-compatible JavaScript pre-request/test
  script in the QuickJS sandbox. `network` defaults to `offline`; use `fetch`
  only when the policy and client context allow outbound requests.
- `mdok_import_postman` accepts Collection v2.1 JSON text or a local path and
  returns generated Markdown plus the review manifest without overwriting
  files.
- `mdok_version` reports MDOK, curl compatibility, TLS, and QuickJS profile
  versions.

Document tools accept `paths`, `vars`, `secrets`, `allow_hosts`,
`deny_hosts`, `config`, `environment`, `env_files`, `offline`, and
`timeout_secs`. `env_files` is an explicit ordered list of dotenv paths; the
server confines them to the operator-approved read roots (or its working
directory when no roots are configured). Secret
values are placed in a child environment and referenced as `@env:` values
rather than put directly in the child argv. MDOK still redacts secrets in
reports and enforces its configured policy.

## Agent workflow: example to CI

Use the same Markdown path at every stage. A typical MCP handoff looks like
this (the JSON objects are tool arguments, not direct HTTP requests):

| Intent | Agent/artifact action | MCP operation |
| --- | --- | --- |
| Describe intent / show an API example | Create a readable `.md` with request/check fences | `mdok_list`, then `mdok_lint` with `offline: true` |
| Plan, try, or debug the API | Edit that same file and inspect redacted diagnostics | `mdok_plan`, then `mdok_test` |
| Approve expected behavior | Confirm status, response shape, and business assertions with the user | Agent conversation; no network tool call |
| Save the successful interaction | Write the reviewed `.md` into the repository | Workspace file write; MCP does not silently persist files |
| Turn it into a verified workflow | Add captures, dependencies, business checks, and cleanup | `mdok_list` → `mdok_lint` → `mdok_plan` → `mdok_test` |
| Run the contract in CI | Commit the Markdown and publish structured results | CLI `mdok test --json --junit` or the CI's MCP client |

For example, after the agent creates `examples/login.md`, inspect it without
network access:

```json
{
  "paths": ["examples/login.md"],
  "vars": {"base_url": "https://staging.example.test"},
  "offline": true
}
```

Then use the same `paths` and explicit runtime inputs for `mdok_plan` and
`mdok_test`. Put tokens and passwords in `secrets`, not in the Markdown or
`vars`. If a dotenv file is needed, pass it explicitly through `env_files`.

MCP currently exposes inspection, planning, execution, import, probe, and
version tools; it does not expose `record` or `replay` as MCP tools. For those
two persistence operations, use the CLI (or save the Markdown with the host's
workspace tools):

```sh
mdok record --content '...' --output examples/login.md
mdok replay --strict examples/login.md
```

Once the example is accepted, CI can run the committed artifact and retain
machine-readable results:

```sh
mdok test examples/login.md --json --junit target/mdok-login.xml
```

## Recommended agent workflow

1. If the starting point is a Postman collection, use `mdok_import_postman`.
   Treat `requires_review`
   and manifest errors as a stop condition; do not execute a lossy import
   without a human decision.
2. Store the returned Markdown in the repository as the reusable workflow and
   review its captures, checks, variables, and host policy.
3. Use `mdok_list` to understand the workflow, then call `mdok_lint`,
   `mdok_plan`, and `mdok_test` in that order.
4. Use captures and JMESPath checks for end-to-end setup/assertion/cleanup
   flows instead of a chain of opaque shell commands.
5. Keep network access offline by default and pass explicit host patterns for
   trusted test environments.
6. Commit the reviewed Markdown and run it again in CI with JSON/JUnit output.

The CLI remains available when MCP is not configured:

```sh
mdok lint api-flow.md
mdok plan api-flow.md --var base_url=http://127.0.0.1:9800
mdok test api-flow.md --allow-host 127.0.0.1
mdok import postman collection.json --out api-flow.md
```

For a standalone Postman script case, use `mdok probe --case case.json` (or
`--case -` for stdin). The JSON shape is documented in
[QUICKJS_PROBE_SPEC.md](QUICKJS_PROBE_SPEC.md).
