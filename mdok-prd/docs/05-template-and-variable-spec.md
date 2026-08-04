# 5. Templates and Variables

## 5.1 Namespaces and precedence

Variable lookup order during a request:

1. captures from completed earlier steps;
2. CLI `--var` and `--secret` values;
3. selected environment profile;
4. inline `toml mdok vars` blocks;
5. project defaults;
6. built-in read-only values.

Duplicate definitions at the same level are errors. Environment variables from the process are not imported unless explicitly mapped in `mdok.toml` or passed with `--allow-env NAME`.

## 5.2 Template grammar

```ebnf
template       = "{{", wsp*, path, *(wsp*, "|", wsp*, filter), wsp*, "}}" ;
path           = identifier, *(".", identifier | "[", index, "]") ;
filter         = "string" | "raw" | "json" | "url" | "header" | "base64" ;
```

Templates are parsed into the Bash word AST. They are not implemented by global string replacement.

## 5.3 Filters

| Filter | Meaning |
|---|---|
| `string` | Scalar to UTF-8 string. Default. Objects/arrays are type errors. |
| `raw` | Exact scalar string with no additional encoding; still one argv value. |
| `json` | Canonical JSON serialization suitable for JSON bodies. |
| `url` | RFC 3986 percent-encoding for a path/query component. |
| `header` | String with CR and LF forbidden; prevents header injection. |
| `base64` | Standard Base64 encoding of string/bytes. |

`{{value}}` is equivalent to `{{value|string}}`. No filter causes shell evaluation.

## 5.4 Quoting model

Quotes belong to the shell source and are removed while building argv. Template values become data inside the resulting argument. A value containing quote characters, spaces, semicolons, `$()`, or newlines cannot create a new argument or command.

Example:

```curl
curl --header "X-Display: {{display_name|header}}" "{{base_url}}/me"
```

If `display_name` is `W \"Admin\"`, the header argument remains one argv element. If it contains CR or LF, the header filter fails before execution.

## 5.5 Secret declarations

Project configuration may declare secret sources:

```toml
[env.staging.secrets]
api_token = { from_env = "STAGING_API_TOKEN" }
```

CLI:

```bash
mdok test api.md --secret api_token=@prompt
mdok test api.md --secret api_token=@file:token.txt
```

Interactive prompts are prohibited in CI mode.

## 5.6 Captured variable lifecycle

- Captures are document-run scoped.
- They are cleared between retries of the whole document.
- A failed source step publishes no captures.
- Capture objects are immutable after publication.
- Secrets can be marked by project policy paths, for example `body.access_token`.
