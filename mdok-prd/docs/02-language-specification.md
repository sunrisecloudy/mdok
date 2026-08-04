# 2. MDOK Language Specification

## 2.1 File recognition

Any UTF-8 Markdown file may contain MDOK blocks. `.mdok.md` is recommended but not required. Invalid UTF-8 is a parse error. A UTF-8 BOM is accepted and excluded from source columns.

## 2.2 Executable fence forms

### Variables

````markdown
```toml mdok vars
base_url = "http://127.0.0.1:9800"
user = "alice"
```
````

The content is parsed by a TOML parser. The resulting root must be a table. Inline variables are document-scoped and immutable after planning. Captures live in a separate runtime namespace.

### Request

````markdown
```curl mdok name=create_user
curl --request POST "{{base_url}}/users" \
  --header "Content-Type: application/json" \
  --data-raw '{"name":{{user|json}}}'
```
````

`name` is required and must be unique in a document. A request fence contains exactly one curl simple command. The leading word must be `curl`; omitting it is not supported in version 1 because copied commands should remain executable outside MDOK.

### Checks

````markdown
```jmespath mdok check=create_user
status == `201`
body.name == variables.user
length(headers."content-type") == `1`
```
````

Each non-empty line is one complete standard JMESPath expression. Each expression must evaluate to boolean `true`. Blank lines are ignored. JMESPath comments are not invented; explanation belongs in surrounding Markdown.

### Capture

````markdown
```jmespath mdok capture=create_user
{id: body.id, etag: headers.etag[0]}
```
````

A capture fence contains one complete JMESPath expression. The result must be an object. Each top-level key becomes a captured variable after all checks associated with the source step have passed. Null values are allowed unless project policy forbids them.

## 2.3 Fence metadata grammar

The CommonMark info string is parsed after Markdown parsing. It uses a restricted argument grammar:

```ebnf
info-string   = language, 1*space, "mdok", *(1*space, attribute) ;
language      = identifier ;
attribute     = flag | key, "=", value ;
flag          = identifier ;
key           = identifier ;
value         = bare-value | single-quoted | double-quoted ;
identifier    = letter, *(letter | digit | "_" | "-") ;
bare-value    = 1*(unreserved) ;
```

Duplicate attributes, unknown required attributes, malformed quoting, and conflicting block roles are planning errors.

## 2.4 Step identifiers

```text
^[A-Za-z][A-Za-z0-9_-]{0,63}$
```

Identifiers are case-sensitive. Reserved names include `variables`, `steps`, `environment`, `request`, `response`, and `mdok`.

## 2.5 Association and order

- Requests execute in document order.
- A check or capture may refer only to a request step defined earlier in the document in version 1.
- Multiple check fences may target one step; their expressions are evaluated in source order.
- Multiple capture fences may target one step; keys must not collide unless `allow_capture_override=true` is explicitly configured.
- Captures become available only after the source step's transfer and checks succeed.
- A request that references an unavailable capture fails before network execution.

## 2.6 Ignored Markdown

Normal prose, headings, lists, tables, links, images, HTML, inline code, and code fences without the `mdok` marker are not executed. They remain part of source context for diagnostics.

## 2.7 Language version

`mdok.toml` declares the language version:

```toml
language = "1"
curl_compat = "8.21"
```

A document may override only with an explicit HTML metadata comment in a future version; version 1 uses project configuration to avoid front-matter ambiguity.
