# Durable agent command tests

MDOK keeps `curl` as the default request adapter and also supports an
explicit `exec` adapter for repository-local agent tools. An `exec` fence is a
single, tokenized argv command:

````markdown
```exec mdok name=agent_message
mdok-command-fixture json
```

```jmespath mdok check=agent_message
exit_code == `0`
```
````

The command is never passed to a shell. Pipes, redirects, substitutions,
assignments, glob expansion, shell interpreters, and stdin are unavailable.
The executable must be selected by a trusted command profile in the project's
policy. The profile key is the first argv token; the configured program path is
resolved relative to `mdok.toml`, canonicalized, and executed directly. No
ambient `PATH` lookup is performed:

```toml
[execution]
command_timeout = "30s"
max_command_output_bytes = 1048576
max_command_args = 64
max_command_arg_bytes = 65536
max_command_argv_bytes = 1048576

[policy.exec]
enabled = true

[policy.exec.commands.mdok-command-fixture]
program = "../../target/debug/mdok-command-fixture"
```

The process environment starts empty. A profile may declare fixed environment
values and explicitly map secret variables into named environment variables:

````toml
[policy.exec.commands.api_check]
program = "tools/api-check"
env = { LC_ALL = "C" }
secret_env = { API_TOKEN = "api_token" }
````

Secrets are not permitted in an `exec` argv template. Output is bounded by one
combined stdout/stderr budget before it enters the JMESPath context. The
context fields are:

| Field | Meaning |
| --- | --- |
| `success` | Process exited zero without timeout or output-limit termination. |
| `exit_code` | Numeric exit status, or `null` when unavailable. |
| `timed_out` | Whether the configured timeout stopped the process. |
| `kind` | Always `exec` for an external command step. |
| `stdout` / `stderr` | Bounded UTF-8-lossy command output. |
| `stdout_json` | Parsed stdout when it is valid JSON, otherwise `null`. |
| `stdout_bytes` / `stderr_bytes` | Bytes retained from each stream. |
| `output_truncated` | Whether the combined output budget was reached. |
| `secret_tainted` | Whether a declared secret environment was present; tainted output cannot be captured. |
| `duration_ms` | Measured process duration. |

Store durable command tests as Markdown under a reviewed repository directory,
for example `tests/agent-commands/`. They are ordinary versioned test assets:
an agent can add a named command, run the directory with `mdok test`, review
the diff, and retain the test for future sessions. The checked-in example is
[`tests/agent-commands/stored-command.md`](/Users/vehasuwat/Project/mdok/tests/agent-commands/stored-command.md).
Build the deterministic fixture first with `cargo build -p mdok-command-fixture`,
then run the stored suite with:

````sh
cargo run -p mdok-cli -- --config tests/agent-commands/mdok.toml test tests/agent-commands
````

An `exec` step with a nonzero exit or timeout fails the step even when its
output checks are present. This prevents a command test from passing merely
because it produced inspectable output; use a successful command and assert
its output for the normal case.
