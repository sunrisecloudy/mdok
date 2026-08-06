# description

> Reference page for the `description` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

A Description can be a raw text, or be an object, which holds the description along with its format.

## Reference table

### Description

| Property | Type | Description |
| --- | --- | --- |
| `content` | `string` | The content of the description goes here, as a raw string. |
| `type` | `string` | Holds the mime type of the raw description content. E.g: 'text/markdown' or 'text/html'.
The type is used to correctly render the description when generating documentation, or in the Postman app. |
| `version` | `any` | Description can have versions associated with it, which should be put in this property. |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/description",
  "description": "A Description can be a raw text, or be an object, which holds the description along with its format.",
  "oneOf": [
    {
      "type": "object",
      "title": "Description",
      "properties": {
        "content": {
          "type": "string",
          "description": "The content of the description goes here, as a raw string."
        },
        "type": {
          "type": "string",
          "description": "Holds the mime type of the raw description content. E.g: 'text/markdown' or 'text/html'.\nThe type is used to correctly render the description when generating documentation, or in the Postman app."
        },
        "version": {
          "description": "Description can have versions associated with it, which should be put in this property."
        }
      }
    },
    {
      "type": "string"
    },
    {
      "type": "null"
    }
  ]
}
```

