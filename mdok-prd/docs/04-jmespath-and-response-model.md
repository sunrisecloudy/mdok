# 4. JMESPath and Structured Transfer Results

## 4.1 Strict standard JMESPath

Version 1 uses the standard JMESPath grammar and built-in functions. MDOK does not silently add regex, date, schema, or assertion functions. Expressions are compiled during planning and reused at execution.

## 4.2 Check semantics

For each non-empty check line:

- parse failure -> `MDOK-E500`;
- runtime/type failure -> `MDOK-E501`;
- result `true` -> pass;
- result `false` -> `MDOK-E502` assertion failure;
- result `null` or any non-boolean -> `MDOK-E501` type failure.

Checks are not skipped because of later checks. By default all checks for a completed step run, so one response can report multiple failures. `--fail-fast` changes this.

## 4.3 Capture semantics

A capture fence is compiled as one expression. It must evaluate to an object:

```jmespath
{token: body.access_token, ids: body.items[].id}
```

Non-object results fail with `MDOK-E503`. Keys must be valid variable names. Captured arrays and objects remain typed in the variable store.

## 4.4 Evaluation object

Every check and capture sees this stable object:

```json
{
  "status": 200,
  "method": "POST",
  "url": "https://api.example.test/login",
  "effective_url": "https://api.example.test/login",
  "http_version": "2",
  "headers": {
    "content-type": ["application/json"],
    "set-cookie": ["a=1; Path=/", "b=2; Path=/"]
  },
  "body": {"access_token": "secret", "user": {"id": "u_1"}},
  "body_text": "{...}",
  "body_base64": null,
  "body_kind": "json",
  "cookies": [{"name": "a", "value": "1", "domain": "api.example.test"}],
  "redirects": [],
  "timings": {
    "queue_ms": 0.0,
    "dns_ms": 1.2,
    "connect_ms": 3.4,
    "tls_ms": 12.0,
    "ttfb_ms": 20.1,
    "total_ms": 21.4,
    "redirect_ms": 0.0
  },
  "transfer": {
    "uploaded_bytes": 54,
    "downloaded_bytes": 91,
    "request_header_bytes": 245,
    "response_header_bytes": 188,
    "primary_ip": "127.0.0.1",
    "primary_port": 9800,
    "local_ip": "127.0.0.1",
    "local_port": 53124,
    "redirect_count": 0,
    "used_proxy": false
  },
  "tls": {
    "verified": true,
    "verify_result": 0
  },
  "error": null,
  "variables": {
    "email": "agent@example.com",
    "user_id": "u_1"
  },
  "steps": {
    "login": {"status": 200}
  }
}
```

## 4.5 Header model

Header names are ASCII-lowercased. Values are arrays preserving receive order. Duplicate headers are never comma-joined because that is invalid for fields such as `set-cookie`. Interim `1xx` response headers and redirect-hop headers are retained in `redirects`; `headers` represents the final response.

## 4.6 Body model

- Valid JSON is exposed as typed `body`; `body_kind="json"`.
- Valid UTF-8 non-JSON text is exposed as string `body` and `body_text`; `body_kind="text"`.
- Binary data has `body=null`, `body_text=null`, and optional bounded `body_base64`; `body_kind="binary"`.
- Empty body has `body=null`, `body_text=""`, and `body_kind="empty"`.
- JSON parsing uses bytes after content decoding performed by libcurl.
- A policy controls whether content type is required for JSON detection; default is content type or successful JSON parse.

## 4.7 Secret taint

JMESPath evaluation operates on real values, but reporting wraps values with taint metadata. Any result derived from a secret variable or secret response field is redacted when formatted. Taint is conservative: concatenation, selection, object construction, and array projection preserve taint.
