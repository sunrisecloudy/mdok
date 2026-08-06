# Collection Version

> Reference page for the `version` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Postman allows you to version your collections as they grow, and this field holds the version number. While optional, it is recommended that you use this field to its fullest extent!

## Reference table

### Collection Version

| Property | Type | Description |
| --- | --- | --- |
| `major` | `integer` | Increment this number if you make changes to the collection that changes its behaviour. E.g: Removing or adding new test scripts. (partly or completely). |
| `minor` | `integer` | You should increment this number if you make changes that will not break anything that uses the collection. E.g: removing a folder. |
| `patch` | `integer` | Ideally, minor changes to a collection should result in the increment of this number. |
| `identifier` | `string` | A human friendly identifier to make sense of the version numbers. E.g: 'beta-3' |
| `meta` | `any` |  |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/version",
  "title": "Collection Version",
  "description": "Postman allows you to version your collections as they grow, and this field holds the version number. While optional, it is recommended that you use this field to its fullest extent!",
  "oneOf": [
    {
      "type": "object",
      "properties": {
        "major": {
          "description": "Increment this number if you make changes to the collection that changes its behaviour. E.g: Removing or adding new test scripts. (partly or completely).",
          "minimum": 0,
          "type": "integer"
        },
        "minor": {
          "description": "You should increment this number if you make changes that will not break anything that uses the collection. E.g: removing a folder.",
          "minimum": 0,
          "type": "integer"
        },
        "patch": {
          "description": "Ideally, minor changes to a collection should result in the increment of this number.",
          "minimum": 0,
          "type": "integer"
        },
        "identifier": {
          "description": "A human friendly identifier to make sense of the version numbers. E.g: 'beta-3'",
          "type": "string",
          "maxLength": 10
        },
        "meta": {}
      },
      "required": [
        "major",
        "minor",
        "patch"
      ]
    },
    {
      "type": "string"
    }
  ]
}
```

