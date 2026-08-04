# 10. Security and Sandboxing

## 10.1 Threat model

An MDOK document may be untrusted and can attempt SSRF, local file disclosure, credential leakage, denial of service, unsafe redirects, proxy abuse, DNS rebinding, path traversal, or parser/resource exhaustion.

## 10.2 Default posture

- No shell execution.
- HTTP/HTTPS only.
- Implicit curl config disabled.
- Interactive credential prompts disabled.
- Local file reads denied unless inside allowlisted project paths.
- File writes denied in version 1 except private temporary spooling and explicit report paths.
- Proxy use denied unless configured.
- Unix sockets denied unless configured.
- `--insecure` denied unless policy allows it.
- Redirects are rechecked against host/scheme policy on every hop.
- Link-local, loopback, private, and metadata addresses can be independently denied.
- DNS answers are checked at connect time, not only URL parse time.

## 10.3 SSRF policy

Host policy supports exact hosts and anchored wildcard suffixes. Resolved IP policy supports CIDRs. Both the hostname and every resolved/connected address must pass. Redirect and `--resolve`/`--connect-to` targets are checked. Cloud metadata ranges are denied by default outside explicit local-test mode.

## 10.4 Filesystem policy

All curl options that reference files are normalized relative to the document/project root, canonicalized without following unsafe symlink escapes, and checked against read/write glob policies. `@-`, `/dev/*`, device paths, named pipes, and Windows device namespaces are denied by default.

## 10.5 Secrets

- CLI secret values are never included in process titles beyond unavoidable user invocation; `@file` and environment mapping are preferred.
- Arguments passed to the in-process C parser do not create a child process.
- Reports redact exact secret values and derived tainted values.
- Request headers commonly carrying credentials are redacted by name.
- Debug traces require explicit opt-in and remain redacted by default.
- Temporary files use owner-only permissions and are unlinked promptly.

## 10.6 Resource limits

- Maximum source bytes per document.
- Maximum AST nodes and executable blocks.
- Maximum argv elements and bytes.
- Maximum template count and expansion bytes.
- Maximum request body and upload bytes.
- Maximum response headers, individual header size, body bytes, redirects, retries, and total time.
- Maximum JSON nesting and JMESPath output size.
- Maximum concurrent documents and open files.

## 10.7 Supply-chain controls

- Pinned curl source checksum and provenance.
- `cargo vet` or equivalent dependency review policy.
- Locked dependencies for release.
- SBOM generation.
- Reproducible-build checks where practical.
- Signed release checksums.
