# Header

> Reference page for the `header` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Represents a single HTTP Header

## Reference table

### Header

| Property | Type | Description |
| --- | --- | --- |
| `key` | `string` | This holds the LHS of the HTTP Header, e.g ``Content-Type`` or ``X-Custom-Header`` |
| `value` | `string` | The value (or the RHS) of the Header is stored in this field. |
| `disabled` | `boolean` | If set to true, the current header will not be sent with requests. |
| `description` | `object | string | null` | A Description can be a raw text, or be an object, which holds the description along with its format. |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "title": "Header",
  "id": "#/definitions/header",
  "description": "Represents a single HTTP Header",
  "properties": {
    "key": {
      "description": "This holds the LHS of the HTTP Header, e.g ``Content-Type`` or ``X-Custom-Header``",
      "type": "string"
    },
    "value": {
      "type": "string",
      "description": "The value (or the RHS) of the Header is stored in this field."
    },
    "disabled": {
      "type": "boolean",
      "default": false,
      "description": "If set to true, the current header will not be sent with requests."
    },
    "description": {
      "$ref": "#/definitions/description"
    }
  },
  "required": [
    "key",
    "value"
  ]
}
```

