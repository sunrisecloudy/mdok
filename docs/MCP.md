# MDOK MCP server

`mdok mcp serve` exposes MDOK as a [Model Context Protocol](https://modelcontextprotocol.io/)
server over stdin/stdout. It is intended for coding agents that need to replace
ad-hoc `curl` calls with reviewable, repeatable API workflows.

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
`deny_hosts`, `config`, `environment`, `offline`, and `timeout_secs`. Secret
values are placed in a child environment and referenced as `@env:` values
rather than put directly in the child argv. MDOK still redacts secrets in
reports and enforces its configured policy.

## Recommended agent workflow

1. Use `mdok_import_postman` for an existing collection. Treat `requires_review`
   and manifest errors as a stop condition; do not execute a lossy import
   without a human decision.
2. Store the returned Markdown in the repository as the reusable workflow and
   review its captures, checks, variables, and host policy.
3. Call `mdok_lint`, then `mdok_plan`, before `mdok_test`.
4. Use captures and JMESPath checks for end-to-end setup/assertion/cleanup
   flows instead of a chain of opaque shell commands.
5. Keep network access offline by default and pass explicit host patterns for
   trusted test environments.

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
