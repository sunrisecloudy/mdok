# MDOK agent workflow

MDOK is designed around one durable Markdown artifact. The agent can begin
with a plain-language request and finish with the same file running in CI.

## The journey

1. **Describe intent and show an API example.** Create a readable `.md` file
   with variables, a named request, and the smallest useful expected-response
   checks.
2. **Plan, try, or debug it.** Inspect the document, lint it, plan it, and run
   it. When a check fails, edit the same Markdown file and rerun it; do not
   replace the workflow with a one-off raw request.
3. **Approve expected behavior.** Ask the user to confirm the status, response
   shape, and business assertions before treating them as a contract.
4. **Save the successful interaction.** Write the reviewed Markdown beside the
   code. For transient or direct input, the CLI can create a replayable file:

   ```sh
   mdok record --content '...' --output examples/api.md
   mdok replay --strict examples/api.md
   ```

5. **Turn it into a verified workflow.** Add captures, dependent requests,
   setup/cleanup, business assertions, and explicit host, timeout, secret, and
   environment-file policy.
6. **Run the same artifact in CI.** Keep one source of truth and retain
   structured evidence:

   ```sh
   mdok test examples/api.md --json --junit target/mdok-api.xml
   ```

## MCP handoff

When MCP is available, the recommended order is:

```text
mdok_list → mdok_lint → mdok_plan → mdok_test
```

The agent or workspace owns creating and saving the Markdown file. MCP tools
inspect, plan, and execute it; they do not silently persist edits. Pass runtime
values through structured `vars`, `secrets`, and explicit `env_files` inputs.
Never put credentials in the committed example.

See [MCP.md](MCP.md) for the tool schemas, operator policy, and the boundary
between MCP operations and CLI-only `record`/`replay`.

## Review boundary

Before a live run, review the redacted plan. Confirm:

- the destination scheme and host are expected;
- variables and dotenv files are explicit;
- secrets are passed through secret inputs and are not in Markdown;
- checks assert the behavior the user actually wants; and
- dependent or mutating steps have safe ordering and cleanup.

For the reusable agent instructions, see
[`skills/mdok/SKILL.md`](../skills/mdok/SKILL.md).
