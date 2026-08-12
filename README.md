# MDOK

> Develop APIs with humans and AI agents. Use Markdown.

[![Release](https://img.shields.io/github/v/release/sunrisecloudy/mdok)](https://github.com/sunrisecloudy/mdok/releases/latest)
[![CI](https://github.com/sunrisecloudy/mdok/actions/workflows/ci.yml/badge.svg)](https://github.com/sunrisecloudy/mdok/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Use MDOK with your AI agent

Run the three setup commands below. Then open Codex, Claude Code, Cursor, or
another coding agent. Give the agent this instruction:

```text
Use MDOK to design a login API for this project.
Create a readable Markdown example.
Lint the file and show me the execution plan.
Ask for my approval before you call the API.
If the test passes, add checks and save the workflow for CI.
```

The agent creates `api/login.md` in your repository. Then the agent uses MDOK
through MCP to examine and run the file:

```text
You describe the API
        ↓
The agent writes a readable .md example
        ↓
MDOK lint → plan → your approval → test
        ↓
The agent finds failures and updates the same file
        ↓
The approved example becomes a verified CI contract
```

You review the Markdown file. You approve the destination and the expected
behavior. MDOK applies execution limits and an explicit host policy. It also
removes secrets from reports. The workflow stays beside your code and does not
disappear into the chat history.

## Install the complete agent-native setup

For [Codex](https://github.com/openai/codex), run these three commands:

```sh
brew install sunrisecloudy/tap/mdok
codex mcp add mdok -- mdok mcp serve
npx -y skills add sunrisecloudy/mdok --skill mdok --agent codex --global --yes
```

These commands install one MDOK binary. Homebrew installs the CLI and its MCP
server. The Agent Skill gives the workflow instructions to the agent.

For Claude Code, run these commands:

```sh
brew install sunrisecloudy/tap/mdok
claude mcp add --scope user mdok -- mdok mcp serve
npx -y skills add sunrisecloudy/mdok --skill mdok --agent claude-code --global --yes
```

You can also install the skill for Cursor, GitHub Copilot, Cline, and other
supported coding agents. Run the installer without `--agent`. Then select an
agent from the list:

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

Use these commands to lint, plan, and test a workflow:

```sh
mdok lint api.md
mdok plan api.md --offline
mdok test api.md --allow-host api.example.com
```

Use `--json`, `--json-lines`, or `--junit` for agents and CI. Pass non-secret
values with `--var`. Pass credentials with `--secret`. Load a dotenv file only
when you name it with `--env-file`.

### MCP server

The Homebrew package includes the stdio MCP server:

```sh
mdok mcp serve
```

Add this JSON to an MCP client:

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

The agent can use `mdok_list`, `mdok_lint`, `mdok_plan`, `mdok_test`,
`mdok_probe`, `mdok_import_postman`, and `mdok_version`. MDOK returns structured
plans and reports. MDOK removes secrets from this output. You control the
allowed hosts, secrets, timeouts, and file access.

### Agent Skill

Install the skill globally for your coding agent:

```sh
npx -y skills add sunrisecloudy/mdok --skill mdok --global
```

Then ask naturally:

```text
Design a login API example with MDOK.
Show me the execution plan.
Ask for my approval before you run it against staging.
Turn the successful flow into a verified workflow for CI.
```

The skill tells the agent how to create the Markdown file. The agent examines
the file before a network call. The agent keeps secrets outside the document,
finds failures, and adds captures and checks. CI uses the approved workflow.

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

Add another request. Capture an ID or token with JMESPath. Use the captured
value in a later step. The result is a Markdown file. A person can read it in a
pull request. The CLI, MCP, an agent, or CI can run the same file.

## Five everyday API-development use cases

MDOK supports five common Postman and Bruno workflows. MDOK keeps the result in
one Markdown file that an agent can read.

### 1. Send, inspect, and debug API requests

Ask the agent to create the smallest useful request. The agent lints the file
and shows the plan before execution. MDOK removes secrets from the plan. After
your approval, run `mdok_test` through MCP or `mdok test` from the CLI. MDOK
returns the status, headers, body, timing, checks, and diagnostic data.

```text
Use MDOK to call the staging health endpoint.
Show me the plan first.
Ask for my approval before the call.
If the call fails, find the cause in MDOK. Do not use a raw API call.
```

### 2. Test and validate API behavior

Put the expected status in `jmespath mdok` check fences. Also put the expected
response structure and business rules in these fences. A workflow passes only
when all requests and checks pass.

```sh
mdok lint api.md
mdok test api.md --allow-host staging.example.com --json
```

### 3. Manage reusable workflows, environments, and authentication

Put `.md` workflows in the repository directories. Use `mdok.toml`, `--var`,
and a named `--env-file` for local, staging, and CI runs. Keep tokens and
passwords out of Markdown. Pass them with `--secret` or MCP `secrets`.

```sh
mdok test api/login.md \
  --env-file .env.staging \
  --secret API_TOKEN=@env:API_TOKEN \
  --allow-host staging.example.com
```

### 4. Share API examples and documentation through Git

The executable example uses standard Markdown. Commit the file beside the
code. Review requests and checks in pull requests. Render the file on any Git
host. The next developer or agent can run the same file. You do not need a
separate documentation export or cloud workspace.

```text
Use MDOK to document the user-creation API.
Include one successful example and useful checks.
Save the file as docs/api/users.md.
```

### 5. Automate regression testing and CI

Run the approved Markdown file in a build, scheduled job, or external monitor.
MDOK can return text, JSON, JSON Lines, or JUnit.

```sh
mdok test docs/api --json --junit target/mdok-api.xml
```

MDOK does not host a mock server or scheduling service. Point the workflow at
your local mock server. Use GitHub Actions or another scheduler for repeated
monitoring. Use the Markdown contract and its results as the approved record.

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

Choose Postman when you want a large visual API platform and cloud
collaboration. Choose Bruno when you want an offline desktop client with
Git-native collections. Choose MDOK for API development with agents. An agent
can create the workflow. A person can examine it. CI can run the same file.

MDOK can also import a Postman Collection v2.1 into reviewable Markdown:

```sh
mdok import postman collection.json --out api.md
```

The importer identifies behavior that needs human review. It does not claim
that each collection feature has an exact Markdown form.

Comparison references:

- [Postman workspaces](https://learning.postman.com/docs/collaborating-in-postman/using-workspaces/overview/)
- [Postman Native Git](https://learning.postman.com/docs/use/native-git/overview/)
- [Postman CLI](https://learning.postman.com/docs/postman-cli/postman-cli-collections)
- [Postman Agent Mode](https://learning.postman.com/docs/use/agent-mode/overview/)
- [Bruno Git collaboration](https://docs.usebruno.com/git-integration/overview)

## Why Markdown works better for agents

- **Use one file.** The agent creates an example. A person reviews it. CI runs
  it as a contract.
- **See the purpose.** Requests, variables, checks, captures, and documentation
  stay together in the file.
- **Review before execution.** An agent can lint the file and examine a plan
  before a network call. MDOK removes secrets from the plan.
- **Keep the correction.** When the agent corrects a failure, the correction
  stays in the workflow. It does not disappear into chat or terminal history.
- **Set clear limits.** You set the hosts, secrets, dotenv files, timeouts, body
  limits, and trusted command profiles.

## FAQ for users and AI agents

### What is MDOK?

MDOK is a CLI and MCP server. It changes executable Markdown into controlled,
repeatable API workflows. A `.md` file can contain documentation, variables,
requests, checks, captures, and dependent steps.

### When should an AI agent use MDOK?

Use MDOK to design an API or show an API example. Use it to call or debug an
endpoint. You can validate a response or import a Postman collection. You can
also save an API interaction or make an API regression workflow for CI.

### What should the agent do first?

Create or find the Markdown workflow. Then use this order:

```text
mdok_list → mdok_lint → mdok_plan → user approval → mdok_test
```

Before a live request, make sure that the destination is correct. Review the
plan and the expected behavior. Then ask the user for approval.

### How should an agent represent an API example?

Write a named `curl mdok` request fence in a `.md` file. Add at least one useful
`jmespath mdok` check. Use a `toml mdok vars` fence only for non-secret default
values. Add capture fences when a later request needs a value from an earlier
response.

### Is a `curl mdok` fence a shell command?

No. MDOK parses the Markdown input and applies limits. Do not copy it into a
shell. Do not replace the workflow with `curl`, `fetch`, HTTPie, or another raw
request method.

### How should variables and secrets be supplied?

Use CLI `--var` or MCP `vars` for non-secret runtime values. Use CLI `--secret`
or MCP `secrets` for credentials. Name a dotenv path with CLI `--env-file PATH`
or MCP `env_files`. MDOK does not search for `.env` files automatically.

### How does an agent execute a workflow safely?

First, lint the file and make an offline plan. Before `mdok_test`, set the
allowed host and a timeout. Review steps that change data. Also review the
cleanup steps. MDOK applies execution limits and removes secrets from output.

### Can MDOK import Postman collections?

Yes. Use `mdok_import_postman` over MCP or:

```sh
mdok import postman collection.json --out api.md
```

Review the generated Markdown and the import manifest. The importer lists
unsupported features and data that it cannot convert without loss. Review
these items before execution.

### How should an agent debug a failed API workflow?

Read the diagnostic data, the failed request, and the failed check. Update the
same Markdown file. Lint the file and make a new plan. Ask for approval before
the next live run. Do not replace the workflow with a single raw request.

### How does the same workflow run in CI?

Commit the approved `.md` file. Run it with stable runtime inputs:

```sh
mdok test docs/api --json --junit target/mdok-api.xml
```

Store the JSON or JUnit report as a CI artifact. Do not make a second API
definition only for CI.

### Does MDOK replace Postman or Bruno?

No. Each tool has a different purpose. Use Postman when you need its large
visual and cloud platform. Use Bruno when you want an offline desktop client.
Use MDOK for agent-assisted API development with Markdown, Git, plans, and MCP
tools. The same MDOK example can run in CI.

### Where are the complete instructions for an agent?

Install the repository's `mdok` Agent Skill. You can also read
[`skills/mdok/SKILL.md`](skills/mdok/SKILL.md). Use the MCP schemas as the
approved source for current tool arguments.

## Documentation

- [Getting started](docs/GETTING_STARTED.md)
- [Agent workflow](docs/AGENT_WORKFLOW.md)
- [MCP server and tool schemas](docs/MCP.md)
- [Postman migration](docs/POSTMAN_IMPORT.md)
- [Focused Markdown E2E workflows](tests/e2e/INDEX.md)
- [Developer and maintainer guide](docs/DEVELOPMENT.md)
- [Complete documentation index](docs/README.md)

The [MIT License](LICENSE) applies to MDOK.
