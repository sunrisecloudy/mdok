# Event

> Reference page for the `event` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Defines a script associated with an associated event name

## Reference table

### Event

| Property | Type | Description |
| --- | --- | --- |
| `id` | `string` | A unique identifier for the enclosing event. |
| `listen` | `string` | Can be set to `test` or `prerequest` for test scripts or pre-request scripts respectively. |
| `script` | `object` | A script is a snippet of Javascript code that can be used to to perform setup or teardown operations on a particular response. |
| `disabled` | `boolean` | Indicates whether the event is disabled. If absent, the event is assumed to be enabled. |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/event",
  "title": "Event",
  "description": "Defines a script associated with an associated event name",
  "type": "object",
  "properties": {
    "id": {
      "type": "string",
      "description": "A unique identifier for the enclosing event."
    },
    "listen": {
      "type": "string",
      "description": "Can be set to `test` or `prerequest` for test scripts or pre-request scripts respectively."
    },
    "script": {
      "$ref": "#/definitions/script"
    },
    "disabled": {
      "type": "boolean",
      "default": false,
      "description": "Indicates whether the event is disabled. If absent, the event is assumed to be enabled."
    }
  },
  "required": [
    "listen"
  ]
}
```

