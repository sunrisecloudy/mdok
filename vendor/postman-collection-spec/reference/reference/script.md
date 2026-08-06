# Script

> Reference page for the `script` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

A script is a snippet of Javascript code that can be used to to perform setup or teardown operations on a particular response.

## Reference table

### Script

| Property | Type | Description |
| --- | --- | --- |
| `id` | `string` | A unique, user defined identifier that can  be used to refer to this script from requests. |
| `type` | `string` | Type of the script. E.g: 'text/javascript' |
| `exec` | `array | string` |  |
| `src` | `object | string` | If object, contains the complete broken-down URL for this request. If string, contains the literal request URL. |
| `name` | `string` | Script name |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/script",
  "title": "Script",
  "type": "object",
  "description": "A script is a snippet of Javascript code that can be used to to perform setup or teardown operations on a particular response.",
  "properties": {
    "id": {
      "description": "A unique, user defined identifier that can  be used to refer to this script from requests.",
      "type": "string"
    },
    "type": {
      "description": "Type of the script. E.g: 'text/javascript'",
      "type": "string"
    },
    "exec": {
      "oneOf": [
        {
          "type": "array",
          "description": "This is an array of strings, where each line represents a single line of code. Having lines separate makes it possible to easily track changes made to scripts.",
          "items": {
            "type": "string"
          }
        },
        {
          "type": "string"
        }
      ]
    },
    "src": {
      "$ref": "#/definitions/url"
    },
    "name": {
      "type": "string",
      "description": "Script name"
    }
  }
}
```

