# 6. CLI and Configuration

## 6.1 Commands

```text
mdok [test] <PATH...>        Parse, plan, execute, and check.
mdok lint <PATH...>          Parse and statically validate without network access.
mdok plan <PATH...>          Print the normalized execution plan; redact secrets.
mdok list <PATH...>          List documents, steps, checks, and captures.
mdok version                 Print MDOK, curl, libcurl, TLS, and feature versions.
```

`mdok file.md` is an alias for `mdok test file.md`.

## 6.2 Common options

```text
--config <path>              Project configuration; default search is mdok.toml upward.
--env <name>                 Select environment profile.
--var key=value              Set a non-secret variable; repeatable.
--secret key=value           Set a secret; repeatable.
--allow-host <pattern>       Add an allowed destination host.
--deny-host <pattern>        Deny a host even if otherwise allowed.
--jobs <n>                   Parallel documents; steps remain ordered.
--fail-fast                  Stop after first failed assertion/step.
--timeout <duration>         Global per-transfer ceiling.
--max-body <bytes>           Captured body limit.
--json                       Emit one JSON report to stdout.
--json-lines                 Stream event records.
--junit <path>               Write JUnit XML.
--report <path>              Write JSON report atomically.
--no-color                   Disable ANSI output.
--offline                    Deny all network execution; useful with lint/plan.
--seed <u64>                 Deterministic seed for future generators.
```

## 6.3 Exit codes

| Code | Meaning |
|---:|---|
| 0 | All selected documents passed. |
| 1 | One or more checks or transfers failed. |
| 2 | Parse, configuration, or planning error. |
| 3 | Permission/policy denial. |
| 4 | Internal error or invariant failure. |
| 130 | Interrupted by user. |

## 6.4 Configuration

```toml
language = "1"
curl_compat = "8.21"

[execution]
jobs = 4
fail_fast = false
max_body_bytes = 8388608
memory_body_threshold_bytes = 262144
connect_timeout = "5s"
total_timeout = "30s"
command_timeout = "30s"
max_command_output_bytes = 1048576
max_command_args = 64
max_command_arg_bytes = 65536
max_command_argv_bytes = 1048576
allowed_schemes = ["http", "https"]

[policy]
allowed_hosts = ["127.0.0.1", "localhost", "api.example.com"]
allowed_read_paths = ["tests/fixtures/**"]
allowed_write_paths = []
allow_insecure_tls = false
allow_proxy = false
allow_unix_sockets = false

[policy.exec]
enabled = true
working_directory = "tools"

[policy.exec.commands.json-validator]
program = "tools/bin/json-validator"
env = { LC_ALL = "C" }
secret_env = { API_TOKEN = "api_token" }

[vars]
region = "ap-southeast-1"

[env.local.vars]
base_url = "http://127.0.0.1:9800"

[env.staging.vars]
base_url = "https://staging.example.com"

[env.staging.secrets]
api_token = { from_env = "STAGING_API_TOKEN" }
```

`exec` profiles are opt-in. Their `program` paths are resolved relative to the
configuration file, canonicalized, and checked as regular executable files.
Bare command names and ambient `PATH` lookup are not accepted. Environment
variables are cleared before launch; only profile-declared values are passed.

## 6.5 Discovery

Directory input recursively discovers `.md` and `.mdok.md` files, honoring `.gitignore` by default. Hidden directories and `target`, `.git`, `node_modules`, and `vendor` are skipped unless explicitly selected.

## 6.6 Output contract

Human output is concise by default. `--verbose` shows request metadata with secrets redacted. JSON output follows `specs/report.schema.json`. Event ordering is deterministic for sequential runs and includes stable sequence numbers for parallel runs.
