# Postman collection import

MDOK imports Postman Collection v2.1 JSON as a one-way canonicalization step:

```sh
mdok import postman collection.json --out api.mdok.md
```

The command writes a second file, `api.mdok.md.import.json` by default. The
manifest contains the source SHA-256, JSON pointers for generated steps, the
secret-variable list, and every warning or blocking diagnostic. Use
`--manifest PATH` to choose another location. Existing files are never
overwritten unless `--force` is supplied.

Strict mode writes the review manifest but refuses to write Markdown when a
semantic cannot be represented safely. `--allow-lossy` writes the generated
Markdown while retaining those diagnostics; it does not make unsupported
Postman behavior executable.

The first importer covers the portable request subset:

- collection and folder order becomes Markdown headings;
- request methods, URLs, enabled query fields, headers, basic/bearer/API-key
  authentication, raw/urlencoded/form-data/GraphQL bodies, and redirect limits
  become a restricted `curl mdok` block;
- simple status/header/body assertions become JMESPath checks;
- simple response JSON assignments become MDOK captures;
- secret-looking variables are omitted from the generated TOML block, while
  literal secret-looking headers/body values are replaced with placeholders;
  all are listed in the manifest.

The importer reports, rather than silently drops, pre-request JavaScript,
unknown JavaScript, dynamic Postman variables, conflicting variable scopes,
file uploads, unsupported authentication, disabled cookies, insecure TLS,
and other unsupported protocol behavior. Postman environment/data files,
iteration runners, response examples, and arbitrary workflow control flow
remain review work.

## Runtime boundary

The generated Markdown is executed by the existing CLI adapter today. The
`mdok-runtime` crate is not yet the canonical execution engine. Before
promoting import to a runtime-level API, the runtime still needs:

1. a single core `DocumentPlan`/`StepPlan` execution API shared by CLI and
   native hosts;
2. scoped variables and secret taint that preserve collection, environment,
   data, and local precedence without flattening them silently;
3. a policy-controlled JavaScript sandbox, or an explicit permanent decision
   that Postman scripts are not supported;
4. workflow state for iterations, branching, retries, and request-dependent
   control flow;
5. per-step protocol behavior and uniform cookie/session handling across native
   and fallback transports;
6. source spans, import provenance, and structured unsupported-semantics
   diagnostics in the runtime report.
