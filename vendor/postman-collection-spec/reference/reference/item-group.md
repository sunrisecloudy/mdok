# Folder

> Reference page for the `item-group` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

One of the primary goals of Postman is to organize the development of APIs. To this end, it is necessary to be able to group requests together. This can be achived using 'Folders'. A folder just is an ordered set of requests.

## Reference table

### Folder

| Property | Type | Description |
| --- | --- | --- |
| `name` | `string` | A folder's friendly name is defined by this field. You would want to set this field to a value that would allow you to easily identify this folder. |
| `description` | `object | string | null` | A Description can be a raw text, or be an object, which holds the description along with its format. |
| `variable` | `array` | Collection variables allow you to define a set of variables, that are a *part of the collection*, as opposed to environments, which are separate entities.
*Note: Collection variables must not contain any sensitive information.* |
| `item` | `array` | Items are entities which contain an actual HTTP request, and sample responses attached to it. Folders may contain many items. |
| `event` | `array` | Postman allows you to configure scripts to run when specific events occur. These scripts are stored here, and can be referenced in the collection by their ID. |
| `auth` | `auth | null` |  |
| `protocolProfileBehavior` | `object` | Set of configurations used to alter the usual behavior of sending the request |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "title": "Folder",
  "id": "#/definitions/item-group",
  "description": "One of the primary goals of Postman is to organize the development of APIs. To this end, it is necessary to be able to group requests together. This can be achived using 'Folders'. A folder just is an ordered set of requests.",
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "A folder's friendly name is defined by this field. You would want to set this field to a value that would allow you to easily identify this folder."
    },
    "description": {
      "$ref": "#/definitions/description"
    },
    "variable": {
      "$ref": "#/definitions/variable-list"
    },
    "item": {
      "description": "Items are entities which contain an actual HTTP request, and sample responses attached to it. Folders may contain many items.",
      "type": "array",
      "items": {
        "title": "Items",
        "anyOf": [
          {
            "$ref": "#/definitions/item"
          },
          {
            "$ref": "#/definitions/item-group"
          }
        ]
      }
    },
    "event": {
      "$ref": "#/definitions/event-list"
    },
    "auth": {
      "oneOf": [
        {
          "type": "null"
        },
        {
          "$ref": "#/definitions/auth"
        }
      ]
    },
    "protocolProfileBehavior": {
      "$ref": "#/definitions/protocol-profile-behavior"
    }
  },
  "required": [
    "item"
  ]
}
```

