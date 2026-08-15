# MCP conformance fixtures

Inputs for `scripts/run_mcp_conformance.py`. The runner assembles an isolated
temporary workspace from these fixtures (plus a generated operator
`mdok.toml`), starts `<server> mcp serve` inside it, and drives the stdio
JSON-RPC session described in `docs/MCP.md`.

The suite is deliberately binary-agnostic: it validates the wire contract
(initialize handshake, tool inventory, document tools, probe sandbox, Postman
import, and the F5/F7/F9 operator-policy invariants) against any server
binary that implements the mdok MCP surface, so a re-implementation (for
example a Go port) is gated by the identical suite.

- `health.md` — valid workflow for lint/plan/list (offline; never executed).
- `exec.md` — exec-fence workflow (via `mdok-command-fixture`) for a passing
  `mdok test` child run without any network.
- `denied.md` — request to a host outside the operator allowlist; the child
  must fail with a policy diagnostic rather than succeed.
- `postman-minimal.json` — smallest valid Postman Collection v2.1 for the
  import tool.
