# Proxy Config

> Reference page for the `proxy-config` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Using the Proxy, you can configure your custom proxy into the postman for particular url match

## Reference table

### Proxy Config

| Property | Type | Description |
| --- | --- | --- |
| `match` | `string` | The Url match for which the proxy config is defined |
| `host` | `string` | The proxy server host |
| `port` | `integer` | The proxy server port |
| `tunnel` | `boolean` | The tunneling details for the proxy config |
| `disabled` | `boolean` | When set to true, ignores this proxy configuration entity |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/proxy-config",
  "title": "Proxy Config",
  "description": "Using the Proxy, you can configure your custom proxy into the postman for particular url match",
  "type": "object",
  "properties": {
    "match": {
      "default": "http+https://*/*",
      "description": "The Url match for which the proxy config is defined",
      "type": "string"
    },
    "host": {
      "type": "string",
      "description": "The proxy server host"
    },
    "port": {
      "type": "integer",
      "minimum": 0,
      "default": 8080,
      "description": "The proxy server port"
    },
    "tunnel": {
      "description": "The tunneling details for the proxy config",
      "default": false,
      "type": "boolean"
    },
    "disabled": {
      "type": "boolean",
      "default": false,
      "description": "When set to true, ignores this proxy configuration entity"
    }
  }
}
```

