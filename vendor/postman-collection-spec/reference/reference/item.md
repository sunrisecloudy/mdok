# Item

> Reference page for the `item` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Items are entities which contain an actual HTTP request, and sample responses attached to it.

## Reference table

### Item

| Property | Type | Description |
| --- | --- | --- |
| `id` | `string` | A unique ID that is used to identify collections internally |
| `name` | `string` | A human readable identifier for the current item. |
| `description` | `object | string | null` | A Description can be a raw text, or be an object, which holds the description along with its format. |
| `variable` | `array` | Collection variables allow you to define a set of variables, that are a *part of the collection*, as opposed to environments, which are separate entities.
*Note: Collection variables must not contain any sensitive information.* |
| `event` | `array` | Postman allows you to configure scripts to run when specific events occur. These scripts are stored here, and can be referenced in the collection by their ID. |
| `request` | `object | string` | A request represents an HTTP request. If a string, the string is assumed to be the request URL and the method is assumed to be 'GET'. |
| `response` | `array` |  |
| `protocolProfileBehavior` | `object` | Set of configurations used to alter the usual behavior of sending the request |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "title": "Item",
  "id": "#/definitions/item",
  "description": "Items are entities which contain an actual HTTP request, and sample responses attached to it.",
  "properties": {
    "id": {
      "type": "string",
      "description": "A unique ID that is used to identify collections internally"
    },
    "name": {
      "type": "string",
      "description": "A human readable identifier for the current item."
    },
    "description": {
      "$ref": "#/definitions/description"
    },
    "variable": {
      "$ref": "#/definitions/variable-list"
    },
    "event": {
      "$ref": "#/definitions/event-list"
    },
    "request": {
      "$ref": "#/definitions/request"
    },
    "response": {
      "type": "array",
      "title": "Responses",
      "items": {
        "$ref": "#/definitions/response"
      }
    },
    "protocolProfileBehavior": {
      "$ref": "#/definitions/protocol-profile-behavior"
    }
  },
  "required": [
    "request"
  ]
}
```

