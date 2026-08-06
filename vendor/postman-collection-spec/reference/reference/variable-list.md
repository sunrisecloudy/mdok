# Variable List

> Reference page for the `variable-list` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Collection variables allow you to define a set of variables, that are a *part of the collection*, as opposed to environments, which are separate entities.
*Note: Collection variables must not contain any sensitive information.*

## Reference table

_This definition has no direct `properties`; see the schema JSON below for its `oneOf`/`anyOf` structure._
## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/variable-list",
  "title": "Variable List",
  "description": "Collection variables allow you to define a set of variables, that are a *part of the collection*, as opposed to environments, which are separate entities.\n*Note: Collection variables must not contain any sensitive information.*",
  "type": "array",
  "items": {
    "$ref": "#/definitions/variable"
  }
}
```

