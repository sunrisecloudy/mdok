# 17. Deterministic Fixture Server

## 17.1 Startup contract

`mdok-test-server --listen 127.0.0.1:0 --tls-listen 127.0.0.1:0 --json-ready` prints exactly one readiness JSON object:

```json
{"http_base_url":"http://127.0.0.1:43123","https_base_url":"https://127.0.0.1:43124","proxy_url":"http://127.0.0.1:43125","ca_file":"/tmp/mdok-ca.pem"}
```

It then writes logs to stderr in JSON Lines. No public network is required.

## 17.2 Endpoints

| Endpoint | Behavior |
|---|---|
| `/health` | `200 {"ok":true}`. |
| `/echo` | Returns method, path, query, headers, cookies, and parsed/raw body. |
| `/status/{code}` | Returns selected status and deterministic JSON. |
| `/json/{case}` | Nested arrays/objects/nulls/numbers/unicode for JMESPath cases. |
| `/headers` | Duplicate, mixed-case, empty, long, and folded-invalid test variants. |
| `/auth/basic` | Validates fixed basic credentials. |
| `/auth/bearer` | Validates fixed bearer token. |
| `/auth/login` | Returns token/user object for workflow tests. |
| `/users/{id}` | CRUD-like deterministic user endpoints. |
| `/cookies/set` | Sets one or more cookies. |
| `/cookies/echo` | Returns received cookies. |
| `/redirect/{n}` | Redirect chain terminating at `/echo`; can change host/scheme for policy tests. |
| `/delay/{ms}` | Delays before headers. |
| `/stream/{chunks}/{delay_ms}` | Chunked response with deterministic chunks. |
| `/gzip` | Gzip-encoded JSON. |
| `/binary/{size}` | Deterministic non-UTF-8 bytes. |
| `/upload` | Returns size and SHA-256 of uploaded bytes. |
| `/multipart` | Returns parsed fields/files and hashes. |
| `/close/early` | Closes mid-response. |
| `/retry/{failures}` | Fails a deterministic number of times per test key, then succeeds. |
| `/large/{size}` | Bounded generated response for limit/spool tests. |

## 17.3 TLS

Tests use a generated local CA and leaf certificate with IP/DNS SANs. Keys are test-only. The harness provides the CA path; tests must not require `--insecure` except explicit policy cases.

## 17.4 Determinism

The server must not include wall-clock timestamps or random IDs unless a test supplies a seed/key. Per-test mutable state is namespaced by a unique header set by the harness.
