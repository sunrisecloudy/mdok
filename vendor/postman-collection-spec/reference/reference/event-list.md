# Event List

> Reference page for the `event-list` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Postman allows you to configure scripts to run when specific events occur. These scripts are stored here, and can be referenced in the collection by their ID.

## Reference table

_This definition has no direct `properties`; see the schema JSON below for its `oneOf`/`anyOf` structure._
## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/event-list",
  "title": "Event List",
  "type": "array",
  "description": "Postman allows you to configure scripts to run when specific events occur. These scripts are stored here, and can be referenced in the collection by their ID.",
  "items": {
    "$ref": "#/definitions/event"
  }
}
```

