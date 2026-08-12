# MDOK Markdown E2E corpus

Each file in this directory is a runnable Markdown workflow. The focused files
exercise one API-development feature at a time; `combined-workflow.md` shows
how an agent can compose those features into one ordered test.

The runner reads [`manifest.txt`](manifest.txt) as the authoritative file
list. It supplies `base_url` for the HTTP fixture to every file. The TLS case
also requires `https_base_url` and `ca_file` from the fixture server readiness
record. Every request targets the deterministic loopback `mdok-test-server`;
no public network or production credentials are needed.

| File | Focus |
| --- | --- |
| [`01-health-status.md`](01-health-status.md) | Health request, HTTP status, and JSON assertion |
| [`02-template-query.md`](02-template-query.md) | Variable interpolation and URL query encoding |
| [`03-jmespath-capture.md`](03-jmespath-capture.md) | JMESPath checks, capture, and dependent template use |
| [`04-bearer-auth.md`](04-bearer-auth.md) | Bearer Authorization header |
| [`05-json-body.md`](05-json-body.md) | Typed JSON request body |
| [`06-cookie-redirect.md`](06-cookie-redirect.md) | Cookie header and redirect following |
| [`07-retry.md`](07-retry.md) | Bounded transient retry behavior |
| [`08-tls.md`](08-tls.md) | Verified local HTTPS with the generated CA |
| [`combined-workflow.md`](combined-workflow.md) | Health, auth, capture, CRUD, headers, checks, and cleanup |

To run one file manually after starting the fixture server, pass the readiness
values explicitly, for example:

```text
mdok test tests/e2e/01-health-status.md \
  --var base_url=http://127.0.0.1:PORT \
  --allow-host 127.0.0.1
```

The command above is an invocation shape, not a raw HTTP request. Use the
repository E2E runner for the complete manifest so it can start and stop the
fixture and provide the TLS variables safely.
