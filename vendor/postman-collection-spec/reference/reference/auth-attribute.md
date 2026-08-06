# Auth

> Reference page for the `auth-attribute` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Represents an attribute for any authorization method provided by Postman. For example `username` and `password` are set as auth attributes for Basic Authentication method.

## Reference table

### Auth

| Property | Type | Description |
| --- | --- | --- |
| `key` | `string` |  |
| `value` | `any` |  |
| `type` | `string` |  |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "title": "Auth",
  "id": "#/definitions/auth-attribute",
  "description": "Represents an attribute for any authorization method provided by Postman. For example `username` and `password` are set as auth attributes for Basic Authentication method.",
  "properties": {
    "key": {
      "type": "string"
    },
    "value": {},
    "type": {
      "type": "string"
    }
  },
  "required": [
    "key"
  ]
}
```

