# MDOK Product Requirements and Implementation Specification

MDOK turns ordinary Markdown into executable API workflow tests.

The product contract is intentionally small:

- **Markdown** describes the workflow and remains readable documentation.
- **curl syntax** describes each HTTP request.
- **JMESPath** checks and captures values from structured transfer results.
- **Rust** owns Markdown parsing, templates, workflow execution, reports, and the CLI.
- **C** integrates a pinned copy of curl's real command-line parser with libcurl.

This bundle is an implementation-ready specification, not a finished MDOK binary. It contains:

- a complete PRD and language specification;
- architecture and C/Rust FFI details;
- the proposed repository structure and starter interfaces;
- build, security, performance, testing, and release requirements;
- a deterministic fixture-server contract;
- **495 Markdown corpus tests** with a machine-readable manifest;
- scripts to validate and regenerate the corpus.

## Start here

1. Read [`docs/00-product-requirements.md`](docs/00-product-requirements.md).
2. Read [`docs/02-language-specification.md`](docs/02-language-specification.md).
3. Review [`docs/07-architecture.md`](docs/07-architecture.md) and [`docs/08-c-rust-ffi.md`](docs/08-c-rust-ffi.md).
4. Copy `repo-skeleton/` into the implementation repository.
5. Run `python3 scripts/validate_corpus.py` from this bundle.
6. Track implementation in [`docs/20-implementation-checklist.md`](docs/20-implementation-checklist.md).
7. Implement phases in [`docs/15-roadmap-and-acceptance.md`](docs/15-roadmap-and-acceptance.md).

## Canonical example

````markdown
# Authentication

```toml mdok vars
base_url = "http://127.0.0.1:9800"
email = "agent@example.com"
password = "test-password"
```

```curl mdok name=login
curl --request POST "{{base_url}}/auth/login" \
  --header "Content-Type: application/json" \
  --data-raw '{"email":{{email|json}},"password":{{password|json}}}'
```

```jmespath mdok check=login
status == `200`
body.user.email == 'agent@example.com'
type(body.access_token) == 'string'
```

```jmespath mdok capture=login
{access_token: body.access_token, user_id: body.user.id}
```

```curl mdok name=get_profile
curl "{{base_url}}/users/{{user_id|url}}" \
  --header "Authorization: Bearer {{access_token|header}}"
```

```jmespath mdok check=get_profile
status == `200`
body.id == variables.user_id
```
````

Run with either:

```bash
mdok auth.md
mdok test auth.md
```

## Non-goals for version 1

- No new HTTP request DSL.
- No arbitrary shell execution from a `curl` fence.
- No GUI, hosted service, recorder, or proprietary collection format.
- No silent emulation of unsupported curl behavior.
- No non-HTTP protocols in the default policy.
