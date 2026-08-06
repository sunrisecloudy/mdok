# Collection

> Reference page for the top-level collection object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`).

## Reference table

| Property | Type | Description |
| --- | --- | --- |
| `info` | `info` | Detailed description of the info block |
| `item` | `array` | Items are the basic unit for a Postman collection. You can think of them as corresponding to a single API endpoint. Each Item has one request and may have multiple API responses associated with it. |
| `event` | `event-list` | Postman allows you to configure scripts to run when specific events occur. These scripts are stored here, and can be referenced in the collection by their ID. |
| `variable` | `variable-list` | Collection variables allow you to define a set of variables, that are a *part of the collection*, as opposed to environments, which are separate entities.
*Note: Collection variables must not contain any sensitive information.* |
| `auth` | `auth | null` |  |
| `protocolProfileBehavior` | `protocol-profile-behavior` | Set of configurations used to alter the usual behavior of sending the request |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "https://schema.getpostman.com/json/collection/v2.1.0/",
  "type": "object",
  "properties": {
    "info": {
      "$ref": "#/definitions/info"
    },
    "item": {
      "type": "array",
      "description": "Items are the basic unit for a Postman collection. You can think of them as corresponding to a single API endpoint. Each Item has one request and may have multiple API responses associated with it.",
      "items": {
        "title": "Items",
        "oneOf": [
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
    "variable": {
      "$ref": "#/definitions/variable-list"
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
    "info",
    "item"
  ],
  "title": "Collection"
}
```

