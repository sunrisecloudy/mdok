# Certificate

> Reference page for the `certificate` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

A representation of an ssl certificate

## Reference table

### Certificate

| Property | Type | Description |
| --- | --- | --- |
| `name` | `string` | A name for the certificate for user reference |
| `matches` | `array` | A list of Url match pattern strings, to identify Urls this certificate can be used for. |
| `key` | `object` | An object containing path to file containing private key, on the file system |
| `cert` | `object` | An object containing path to file certificate, on the file system |
| `passphrase` | `string` | The passphrase for the certificate |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/certificate",
  "title": "Certificate",
  "description": "A representation of an ssl certificate",
  "type": "object",
  "properties": {
    "name": {
      "description": "A name for the certificate for user reference",
      "type": "string"
    },
    "matches": {
      "description": "A list of Url match pattern strings, to identify Urls this certificate can be used for.",
      "type": "array",
      "items": {
        "type": "string",
        "description": "An Url match pattern string"
      }
    },
    "key": {
      "description": "An object containing path to file containing private key, on the file system",
      "type": "object",
      "properties": {
        "src": {
          "description": "The path to file containing key for certificate, on the file system"
        }
      }
    },
    "cert": {
      "description": "An object containing path to file certificate, on the file system",
      "type": "object",
      "properties": {
        "src": {
          "description": "The path to file containing key for certificate, on the file system"
        }
      }
    },
    "passphrase": {
      "description": "The passphrase for the certificate",
      "type": "string"
    }
  }
}
```

