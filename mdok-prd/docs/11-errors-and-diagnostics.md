# 11. Errors and Diagnostics

## 11.1 Required fields

Every diagnostic contains:

- stable code;
- severity;
- concise title;
- explanatory message;
- source file and smallest useful span;
- document/step/check identifiers when available;
- redacted observed values;
- cause chain for machine output;
- optional repair hint that does not claim certainty.

External command diagnostics use `MDOK-E306` through `MDOK-E312` for policy,
argv, start/reap, exit, timeout, resource-limit, and environment/working-
directory failures. Command stdout/stderr may be available transiently in the
step context for checks, but is not copied into durable reports; secret-tainted
output is redacted and cannot be captured.

## 11.2 Example

```text
error[MDOK-E201]: shell operator is not allowed in a curl block

  tests/auth.md:12:46

12 │ curl "{{base_url}}/me" | jq '.user'
   │                          ^ shell pipelines would execute another program

Step: get_me
Allowed: one simple command whose first word is `curl`
Use a JMESPath check or capture block instead of piping to jq.
```

## 11.3 Agent JSON

```json
{
  "schema_version": "1",
  "code": "MDOK-E502",
  "kind": "assertion_failed",
  "file": "tests/auth.md",
  "step": "login",
  "span": {"byte_start": 413, "byte_end": 428, "line": 19, "column": 1},
  "expression": "status == `200`",
  "result": false,
  "observed": {"status": 401},
  "redactions": ["response.body.access_token"],
  "hint": "Confirm credentials or update the expected status."
}
```

## 11.4 Stability

Error codes and JSON field meanings are compatibility surfaces. Human wording may improve without a major version. Do not make automation depend on full human messages.

See `specs/error-codes.md` for the registry.
