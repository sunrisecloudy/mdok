# MDOK

> API development for humans and AI agents—in Markdown.

[![Release](https://img.shields.io/github/v/release/sunrisecloudy/mdok)](https://github.com/sunrisecloudy/mdok/releases/latest)
[![CI](https://github.com/sunrisecloudy/mdok/actions/workflows/ci.yml/badge.svg)](https://github.com/sunrisecloudy/mdok/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Describe an API once. MDOK turns the same readable `.md` file into an example,
a safe execution plan, a verified end-to-end workflow, and a CI contract.
Your coding agent can design, run, debug, and improve it through MCP instead of
leaving one-off requests scattered through chat history.

```text
intent → Markdown example → safe plan → live test → verified contract → CI
```

## Install the complete agent-native setup

For [Codex](https://github.com/openai/codex), three commands install the CLI,
connect the MCP server, and teach the agent how to use MDOK:

```sh
brew install sunrisecloudy/tap/mdok
codex mcp add mdok -- mdok mcp serve
npx -y skills add sunrisecloudy/mdok --skill mdok --agent codex --global --yes
```

That is one MDOK binary, not three runtimes: Homebrew installs the CLI and its
built-in MCP server; the small Agent Skill supplies the workflow instructions.

Using Claude Code instead?

```sh
brew install sunrisecloudy/tap/mdok
claude mcp add --scope user mdok -- mdok mcp serve
npx -y skills add sunrisecloudy/mdok --skill mdok --agent claude-code --global --yes
```

The skill follows the shared Agent Skills format and can also be installed for
Cursor, GitHub Copilot, Cline, and other supported coding agents. Run the
installer without `--agent` to select one interactively:

```sh
npx -y skills add sunrisecloudy/mdok --skill mdok --global
```

## Choose how you use MDOK

### CLI

Install once:

```sh
brew install sunrisecloudy/tap/mdok
mdok --version
```

Then lint, review, and execute a workflow:

```sh
mdok lint api.md
mdok plan api.md --offline
mdok test api.md --allow-host api.example.com
```

Use `--json`, `--json-lines`, or `--junit` for agents and CI. Pass normal
values with `--var`, credentials with `--secret`, and load a dotenv file only
when you explicitly name it with `--env-file`.

### MCP server

The Homebrew package already includes the stdio MCP server:

```sh
mdok mcp serve
```

For any MCP client that uses JSON configuration:

```json
{
  "mcpServers": {
    "mdok": {
      "command": "mdok",
      "args": ["mcp", "serve"]
    }
  }
}
```

The agent discovers `mdok_list`, `mdok_lint`, `mdok_plan`, `mdok_test`,
`mdok_probe`, `mdok_import_postman`, and `mdok_version`. Plans and reports are
structured and redacted; network hosts, secrets, timeouts, and filesystem
access remain explicit policy decisions.

### Agent Skill

Install the skill globally for your coding agent:

```sh
npx -y skills add sunrisecloudy/mdok --skill mdok --global
```

Then ask naturally:

```text
Design a login API example with MDOK, plan it safely, run it against staging,
and turn the successful flow into a verified workflow for CI.
```

The skill guides the agent to create a durable Markdown file, inspect it before
network execution, keep secrets outside the document, diagnose failures, add
captures and assertions, and reuse the accepted workflow in CI.

## Your first API workflow

Create `api.md`:

````markdown
# Service health

```toml mdok vars
base_url = "https://api.example.com"
```

```curl mdok name=health
curl "{{base_url}}/health"
```

```jmespath mdok check=health
status == `200`
```
````

Review before you run:

```sh
mdok lint api.md
mdok plan api.md --offline
mdok test api.md --allow-host api.example.com
```

Add another request, capture an ID or token with JMESPath, and use it in later
steps. The result is still ordinary Markdown: readable in a pull request,
executable from the CLI, operable through MCP, and reusable by an agent.

## MDOK vs Postman vs Bruno

[Postman](https://www.postman.com/) is a broad collaborative API platform.
[Bruno](https://www.usebruno.com/) is an offline desktop API client with
Git-friendly Bru files. MDOK is built for API development that starts in code
and conversation with an AI agent.

| | Postman | Bruno | MDOK |
| --- | --- | --- | --- |
| Best fit | Full API platform, visual collaboration, mocks, monitors, and testing | Local desktop exploration with offline, Git-native collections | Agent-assisted API design, executable documentation, and repository-native contracts |
| Primary interface | Desktop/web UI, CLI, integrations, and Agent Mode | Desktop UI and Bruno CLI | Markdown, CLI, MCP, and Agent Skill |
| Source artifact | Postman collections and workspace elements; local Git is also supported | Plain-text `.bru` collections in the filesystem | Standard `.md` files beside the code |
| Collaboration model | Cloud workspaces with real-time sync, plus Native Git workflows | Git or another version-control system | Git, code review, and the same artifact used by humans, agents, and CI |
| Agent workflow | Agent Mode and platform/CLI automation | Agents can operate Bru files and the CLI | First-class MCP tools plus a reusable skill that enforces the example-to-CI workflow |
| Execution safety | Collection/runtime settings and platform controls | Client safe mode and collection settings | Offline planning, explicit host allowlists, bounded execution, redacted reports, and explicit dotenv loading |
| CI | Postman CLI, monitors, and cloud/local reports | Bruno CLI and Docker | The same Markdown via `mdok test`, JSON/JSONL/JUnit, or MCP |

Choose Postman when you want an expansive GUI-centered API platform and
cloud collaboration. Choose Bruno when you want a polished offline desktop
client with Git-native collections. Choose MDOK when the API workflow should be
easy for an agent to create, safe to inspect, natural for a human to review,
and ready to run unchanged in CI.

MDOK can also import a Postman Collection v2.1 into reviewable Markdown:

```sh
mdok import postman collection.json --out api.md
```

The importer reports behavior that needs human review instead of silently
pretending every collection feature has an exact Markdown equivalent.

Comparison references: [Postman workspaces](https://learning.postman.com/docs/collaborating-in-postman/using-workspaces/overview/),
[Postman Native Git](https://learning.postman.com/docs/use/native-git/overview/),
[Postman CLI](https://learning.postman.com/docs/postman-cli/postman-cli-collections),
[Postman Agent Mode](https://learning.postman.com/docs/use/agent-mode/overview/),
and [Bruno Git collaboration](https://docs.usebruno.com/git-integration/overview).

## Why Markdown works better for agents

- **One artifact, not a handoff.** The example the agent creates becomes the
  test humans review and the contract CI executes.
- **Intent stays visible.** Requests, variables, assertions, captures, and
  documentation live together instead of behind UI state.
- **Planning is a real step.** An agent can lint and inspect a redacted plan
  before making a network request.
- **Successful debugging becomes durable.** Fixes stay in the workflow rather
  than disappearing into a chat transcript or terminal history.
- **Policy travels with automation.** Hosts, secrets, dotenv files, timeouts,
  body limits, and trusted command profiles remain explicit.

## Documentation

- [Getting started](docs/GETTING_STARTED.md)
- [Agent workflow](docs/AGENT_WORKFLOW.md)
- [MCP server and tool schemas](docs/MCP.md)
- [Postman migration](docs/POSTMAN_IMPORT.md)
- [Focused Markdown E2E workflows](tests/e2e/INDEX.md)
- [Developer and maintainer guide](docs/DEVELOPMENT.md)
- [Complete documentation index](docs/README.md)

MDOK is open source under the [MIT License](LICENSE).
