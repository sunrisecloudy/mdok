# Cookie

> Reference page for the `cookie` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

A Cookie, that follows the [Google Chrome format](https://developer.chrome.com/extensions/cookies)

## Reference table

### Cookie

| Property | Type | Description |
| --- | --- | --- |
| `domain` | `string` | The domain for which this cookie is valid. |
| `expires` | `string | number` | When the cookie expires. |
| `maxAge` | `string` |  |
| `hostOnly` | `boolean` | True if the cookie is a host-only cookie. (i.e. a request's URL domain must exactly match the domain of the cookie). |
| `httpOnly` | `boolean` | Indicates if this cookie is HTTP Only. (if True, the cookie is inaccessible to client-side scripts) |
| `name` | `string` | This is the name of the Cookie. |
| `path` | `string` | The path associated with the Cookie. |
| `secure` | `boolean` | Indicates if the 'secure' flag is set on the Cookie, meaning that it is transmitted over secure connections only. (typically HTTPS) |
| `session` | `boolean` | True if the cookie is a session cookie. |
| `value` | `string` | The value of the Cookie. |
| `extensions` | `array` | Custom attributes for a cookie go here, such as the [Priority Field](https://code.google.com/p/chromium/issues/detail?id=232693) |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "title": "Cookie",
  "id": "#/definitions/cookie",
  "description": "A Cookie, that follows the [Google Chrome format](https://developer.chrome.com/extensions/cookies)",
  "properties": {
    "domain": {
      "type": "string",
      "description": "The domain for which this cookie is valid."
    },
    "expires": {
      "oneOf": [
        {
          "type": "string"
        },
        {
          "type": "number"
        }
      ],
      "description": "When the cookie expires."
    },
    "maxAge": {
      "type": "string"
    },
    "hostOnly": {
      "type": "boolean",
      "description": "True if the cookie is a host-only cookie. (i.e. a request's URL domain must exactly match the domain of the cookie)."
    },
    "httpOnly": {
      "type": "boolean",
      "description": "Indicates if this cookie is HTTP Only. (if True, the cookie is inaccessible to client-side scripts)"
    },
    "name": {
      "type": "string",
      "description": "This is the name of the Cookie."
    },
    "path": {
      "type": "string",
      "description": "The path associated with the Cookie."
    },
    "secure": {
      "type": "boolean",
      "description": "Indicates if the 'secure' flag is set on the Cookie, meaning that it is transmitted over secure connections only. (typically HTTPS)"
    },
    "session": {
      "type": "boolean",
      "description": "True if the cookie is a session cookie."
    },
    "value": {
      "type": "string",
      "description": "The value of the Cookie."
    },
    "extensions": {
      "type": "array",
      "description": "Custom attributes for a cookie go here, such as the [Priority Field](https://code.google.com/p/chromium/issues/detail?id=232693)"
    }
  },
  "required": [
    "domain",
    "path"
  ]
}
```

