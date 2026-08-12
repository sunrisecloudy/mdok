# Agent invocations

MDOK exposes four agent-facing lifecycle commands:

| Command | Input | Default stdout | Durable state |
| --- | --- | --- | --- |
| `run` | Markdown from a path, stdin, or `--content` | The normal MDOK report | None |
| `call` | One direct argv command after `--` | One invocation envelope | None |
| `record` | Markdown or direct argv after `--` | One invocation envelope | Markdown plus a JSON manifest |
| `replay` | A recorded Markdown path | One invocation envelope | None beyond the existing recording |

The JSON envelope is defined by
[`specs/call.schema.json`](../specs/call.schema.json). HTTP response objects
inside the envelope use [`specs/response.schema.json`](../specs/response.schema.json).
`run` is the exception: it uses the normal report contract, including the
existing human, JSON, and JSON Lines report modes. It does not wrap that report
in the invocation envelope.

## Input and command boundaries

Common options can be placed before or after the subcommand. Put them before the
subcommand in scripts so that `--` unambiguously marks the end of MDOK options
and the beginning of direct argv:

```sh
mdok --config ./mdok.toml --env staging --env-file .env.staging run request.md
mdok --allow-host api.example.com call -- curl --fail https://api.example.com/health
```

Anything after the direct-argv separator belongs to the invoked command. For
example, `--offline` is an MDOK option and must appear before `call`; after
`call --` it would be passed to `curl` instead.

Transient Markdown is bounded to 8 MiB. `run` accepts exactly one of a path,
`-`/stdin, or `--content`:

````sh
mdok run request.md
mdok run - < request.md
cat request.md | mdok run
mdok run --content '```curl mdok name=health
curl --fail https://api.example.com/health
```'
````

`call` and direct-argv `record` convert the argv vector into one canonical
Markdown fence. Each argv element remains one data token: spaces, Unicode,
newlines, backslashes, quotes, dollar signs, backticks, and shell operators are
escaped in the generated fence. MDOK does not invoke a shell. The direct argv
limits are 64 arguments, 64 KiB per argument, 1 MiB total argument bytes, and a
512 KiB generated fence.

## `run`: execute transient Markdown

`run` parses and executes the supplied Markdown as a normal one-document MDOK
test. It does not create a recording and it does not emit the invocation
envelope. These are the output forms:

```sh
# Human report (the default)
mdok run request.md

# One complete JSON report
mdok --json run request.md

# JSON Lines report/event stream
mdok --json-lines run request.md

# Read Markdown from stdin and keep the report machine-readable
cat request.md | mdok --json run -
```

`--report PATH` writes the JSON report atomically, and `--junit PATH` writes
JUnit output. `--timeout`, `--max-body`, `--offline`, `--env`, `--env-file`, `--var`,
`--secret`, host policy options, and the other common execution options apply
to the transient run in the same way that they apply to `test`.

`--env-file PATH` is repeatable and never performs automatic discovery. Later
files override earlier files; explicit `--var` and `--secret` assignments win
afterward. Dotenv values are parsed literally without interpolation or command
execution. Secret-looking names are tainted and redacted. Recording provenance
includes each canonical env-file path and digest, so strict replay detects file
changes.

The report exit status is independent of its output format:

| Code | Meaning |
| ---: | --- |
| `0` | The invocation passed. |
| `1` | A transfer, command, or check failed. |
| `2` | Input, parsing, configuration, or planning failed. |
| `3` | A policy or permission check denied the operation. |
| `4` | An internal error occurred. |
| `130` | The process was interrupted. |

## `call`: invoke one direct command

The command after `call --` is converted to a transient one-step Markdown
document and executed immediately. A first token of `curl` selects the HTTP
adapter. Any other first token selects the trusted `exec` adapter and must name
a configured command profile.

```sh
# Direct curl argv; --allow-host is an MDOK option.
mdok --allow-host api.example.com call -- \
  curl --fail --header 'Accept: application/json' \
  https://api.example.com/health

# Direct trusted-profile argv.
mdok --config tests/agent-commands/mdok.toml call -- \
  mdok-command-fixture json

# Raw stdout/body instead of the envelope.
mdok --config tests/agent-commands/mdok.toml call --raw -- \
  mdok-command-fixture echo 'agent value'
```

The default `call` result is one pretty-printed invocation envelope. It includes
the operation (`call`), a source hash, redacted argv, adapter result, execution
metadata, diagnostics, and an exit status derived from the result. It is one
JSON document; `--json` and `--json-lines` do not change this direct invocation
format.

## `record`: persist source, then execute

`record` accepts either transient Markdown or direct argv. It writes the source
before executing it, then returns the normal invocation envelope with recording
metadata:

````sh
# Record Markdown at an explicit path.
mdok --config ./mdok.toml record \
  --output .mdok/records/health.md \
  --content '```curl mdok name=health
curl --fail https://api.example.com/health
```'

# Record a trusted-profile command.
mdok --config tests/agent-commands/mdok.toml record \
  --output .mdok/records/fixture.md -- \
  mdok-command-fixture json

# Record and emit only the response body/stdout.
mdok --config tests/agent-commands/mdok.toml record --raw -- \
  mdok-command-fixture echo 'recorded value'
````

If `--output` is omitted, the Markdown path is generated under
`.mdok/records/` relative to the configuration file's directory, or the
current directory when there is no explicit configuration. A missing extension
is given `.md`. The sibling manifest is `<recording>.md.json` when the recording
is named `<recording>.md`.

Recordings are written atomically with owner-only permissions (`0600` on Unix).
The created recording directory is private (`0700` on Unix), an existing
destination is not replaced unless `--force` is supplied, and a destination
symlink is rejected. The manifest contains the schema version, source SHA-256,
source kind, MDOK/curl versions, creation time, and a provenance snapshot plus
its SHA-256. The snapshot binds the effective configuration, policy, limits,
command profile names and executable hashes, and secret source identifiers; it
does not contain resolved secret values, cookies, authorization headers, or
command environment values.

`record --raw` has the same byte semantics as `call --raw`. `replay` has no
`--raw` option; use the structured result when replay provenance and diagnostics
are needed.

## `replay`: re-execute a recording

Replay reads the recorded Markdown and its sibling JSON manifest, applies the
current MDOK configuration and policy, and executes the request again. It is a
new network or process operation, not a cached response lookup:

```sh
# Replay under the current configuration and policy.
mdok --config ./mdok.toml replay .mdok/records/health.md

# Refuse to execute unless source and provenance match the manifest.
mdok --config ./mdok.toml replay --strict .mdok/records/health.md
```

The replay preflight compares the recording's source SHA-256 and provenance
hash with the current file and configuration. Its status is:

- `exact`: the source and recorded provenance match;
- `changed`: the source, configuration, or secret-input identifiers differ;
- `unknown`: the sibling manifest is missing.

Without `--strict`, replay proceeds for `changed` and `unknown` after the
preflight. With `--strict`, only `exact` proceeds; `changed` and `unknown` fail
with an input error before the adapter runs. A malformed or unsupported
manifest is also an input error. The provenance check does not make a replay
deterministic: remote responses, local files, command binaries, environment
values, and external side effects can still differ. A successful replay
envelope includes the recording and manifest paths, the current source digest,
and the preflight `drift` object so callers can make that decision from one
structured result.

## Invocation envelope

`call` and `record` emit the envelope by default. `replay` emits the structured
envelope when its preflight permits execution; a strict provenance failure is
an input diagnostic and stops before an adapter result exists. The stable
top-level fields are:

| Field | Meaning |
| --- | --- |
| `schema_version` | Envelope version, currently `"1"`. |
| `operation` | `call`, `record`, or `replay` for the corresponding CLI command. |
| `run_id` | A stable run identifier derived from the canonical source digest. |
| `success` | Whether parsing, policy, execution, and checks passed. |
| `result_kind` | `http`, `command`, or `none`. |
| `request` | Adapter, redacted argv, source kind/path, and source SHA-256. |
| `response` | The HTTP evaluation object, command result, or `null`. |
| `execution` | Duration, timeout, exit code, and policy metadata. |
| `recording` | Recording path, manifest path, source digest, and replay command for a newly created recording. |
| `artifacts` | Durable artifact references emitted when `--artifact PATH` is used for an HTTP body. |
| `diagnostics` | Structured errors and warnings. |

`request.argv` is an array of tokens, not a shell command string. Sensitive
headers, URL credentials, and values associated with credential-bearing options
are redacted in the envelope. Consumers must not reparse the array as shell
source.

For `result_kind: "http"`, inspect `response.body_kind` and the corresponding
body field:

- `json`: use `body`;
- `text`: use `body_text`;
- `binary`: use decoded `body_base64` when it is present;
- `empty`: there is no body;
- `truncated`: the retained body is incomplete and must not be treated as a
  complete download.

For `result_kind: "command"`, `response` keeps `stdout` and `stderr` separate,
reports `exit_code`, `timed_out`, output-limit state, retained byte counts, and
`stdout_json` when stdout is valid JSON. A nonzero exit, timeout, or combined
output-limit termination fails the command step even if a check could inspect
the partial output.

`result_kind: "none"` means the adapter did not produce a result. Use
`diagnostics`; there is no response body to interpret.

## Output and `--raw`

`--raw` is available on `call` and `record`, and must appear before the direct
argv separator. It writes only the response payload to stdout:

| Adapter/result | Raw bytes |
| --- | --- |
| HTTP text/JSON | The UTF-8 `body_text` bytes. |
| HTTP binary | The decoded bytes from `body_base64`. |
| Trusted `exec` | `stdout` only; stderr is not mixed into stdout. |
| No adapter result | No payload bytes. |

MDOK adds no envelope, status line, headers, or newline in raw mode. The process
exit code still reports checks, policy, and execution failures. Raw output is
not an instance of `call.schema.json`; a consumer that needs diagnostics must
use structured mode.

Raw output from a secret-tainted trusted command is denied with a policy error.
This prevents a command that received a mapped secret environment value from
being turned into an unreviewed byte capture. Use structured output, which
redacts the configured secret values, or remove the secret mapping.

## Secure recording and replay rules

1. Do not put literal credentials in Markdown, `--content`, or direct argv.
   Prefer `--secret KEY=@env:NAME`, `--secret KEY=@file:PATH`, or a named
   environment secret referenced by a Markdown template. `@prompt` is not
   available in this non-interactive CLI.
2. Direct-argv recording rejects recognized credential-bearing forms, including
   `Authorization:`, `Cookie:`, and `X-Api-Key:` values and the `-u`, `--user`,
   `--cookie`, and `--proxy` options. This is a guard, not a guarantee that an
   arbitrary string is safe; review every recording before committing it.
3. A recording must be valid UTF-8 Markdown. The recorder rejects resolved
   secret values in the source and rejects literal sensitive headers or common
   credential options unless the line uses a `{{...}}` template reference.
   This is still not a complete secret detector: review the source and do not
   record inline literal credentials.
4. Recording preserves the accepted source verbatim. The source file and
   manifest are written with atomic, no-clobber semantics. Keep the recording
   directory private, review both files, and use `--force` only when replacing
   a known recording deliberately.
5. Replay runs the recorded request again under the current configuration. A
   recording path is not a permission grant, and a provenance hash is not proof
   that the current profile binary, network destination, secret value, or
   referenced file is trusted.
6. Use `replay --strict` for a source/configuration/input-integrity gate, then
   separately verify the command profile, referenced files, target host, and
   expected side effects. Never treat replay as a safe read-only operation by
   default.

## Bodies, artifacts, and filesystem policy

Response capture is bounded by `--max-body` or `[execution].max_body_bytes`
(8 MiB by default). Bodies above the memory threshold may spill to an internal
temporary file so that the process does not retain the whole body in memory.
That temporary spool is not a durable artifact and its path is not an agent
access grant.

Durable artifact writes are disabled unless the project explicitly configures
write roots:

```toml
[policy]
allowed_write_paths = ["tests/artifacts/**"]
```

The CLI's `--artifact PATH` option is the direct body-download path for
`call`, `record`, and transient `run` documents. The option is intentionally
not enabled by default; a configured write root is required even when the
destination is local.

An artifact-producing path must remain under a configured write root. The body
must be complete and within the effective body limit; the destination must name
a file; existing destinations are rejected; and the write is copied through a
temporary file and finalized without clobbering. A durable artifact reference
identifies the path, byte count, SHA-256 digest, and completion state. Consumers
must verify the digest and apply their own path/trust policy before opening it.

For a bounded durable HTTP body, pass `--artifact PATH` and configure the
destination's write root explicitly. The path is resolved from the current
working directory, must remain under a configured `allowed_write_paths` root,
and is created without overwriting an existing file:

```sh
mdok --config ./mdok.toml --allow-host api.example.com \
  --artifact .mdok/artifacts/health.json call -- \
  curl --fail https://api.example.com/health
```

The structured response omits the inline body when an artifact is requested and
returns a relative artifact reference with byte count, SHA-256, and
`complete: true`. `--raw` streams the persisted artifact bytes when used with
`--artifact`; it does not mix metadata into stdout. Without `--artifact`, the
body remains in the bounded inline response representation.

## Trusted `exec` profiles

An `exec` fence or a non-`curl` direct `call` is allowed only when its first
argv token names a profile under `[policy.exec.commands]` and execution is
enabled:

```toml
[execution]
command_timeout = "30s"
max_command_output_bytes = 1048576
max_command_args = 64
max_command_arg_bytes = 65536
max_command_argv_bytes = 1048576

[policy.exec]
enabled = true
# Optional; resolved relative to mdok.toml and canonicalized.
working_directory = "tools"

[policy.exec.commands.api_check]
program = "tools/api-check"
env = { LC_ALL = "C" }
secret_env = { API_TOKEN = "api_token" }
```

The profile program is resolved relative to `mdok.toml` when it is not
absolute, canonicalized, required to be a regular executable file, and invoked
directly. MDOK does not perform ambient `PATH` lookup. The child starts with an
empty environment; only fixed `env` entries and explicitly mapped `secret_env`
entries are added. Shell interpreters are rejected as profile commands, and
loader/interpreter injection variables such as `LD_PRELOAD`,
`DYLD_INSERT_LIBRARIES`, `PYTHONPATH`, `NODE_OPTIONS`, and `BASH_ENV` are not
accepted as profile environment names.

Secrets may enter an `exec` process only through `secret_env`; a secret in an
exec argv template is a policy error. Output is bounded before it is exposed to
JMESPath checks or captures. Output from a secret-tainted process cannot be
captured durably, and raw output is denied as described above. See
[`docs/COMMAND_TESTS.md`](COMMAND_TESTS.md) for the durable Markdown command
test format.

## Threat-model checklist

Before running untrusted or newly recorded input, verify:

- [ ] Source: the path/stdin/inline content is the intended bytes; direct argv
      is reviewed as tokens, not as a shell string.
- [ ] Replay: the sibling manifest exists, `replay --strict` is used where
      source integrity matters, and configuration/input drift is reviewed.
- [ ] Secrets: no credentials are in Markdown, shell history, argv, manifests,
      reports, raw logs, or artifacts; secret-tainted exec output is not
      captured or emitted raw.
- [ ] Exec: only required named profiles are enabled; each program, working
      directory, fixed environment entry, and secret mapping is trusted.
- [ ] Network: schemes, hosts, private-network access, proxy/TLS exceptions,
      redirects, timeouts, and replay side effects are explicitly acceptable.
- [ ] Filesystem: read roots and write roots are narrow; artifact paths are
      verified and digests are checked; recording directories are private.
- [ ] Output: structured output is used for diagnostics; raw output is treated
      as untrusted payload bytes and is not mixed with metadata.
- [ ] Repetition: replayed HTTP requests and commands may mutate state; obtain
      idempotence or an explicit approval before re-running them.
