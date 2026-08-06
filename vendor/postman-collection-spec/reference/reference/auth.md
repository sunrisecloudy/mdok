# Auth

> Reference page for the `auth` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

Represents authentication helpers provided by Postman

## Reference table

### Auth

| Property | Type | Description |
| --- | --- | --- |
| `type` | `string` |  — enum: 'apikey', 'awsv4', 'basic', 'bearer', 'digest', 'edgegrid', 'hawk', 'noauth', 'oauth1', 'oauth2', 'ntlm' |
| `noauth` | `any` |  |
| `apikey` | `array` | The attributes for API Key Authentication. |
| `awsv4` | `array` | The attributes for [AWS Auth](http://docs.aws.amazon.com/AmazonS3/latest/dev/RESTAuthentication.html). |
| `basic` | `array` | The attributes for [Basic Authentication](https://en.wikipedia.org/wiki/Basic_access_authentication). |
| `bearer` | `array` | The helper attributes for [Bearer Token Authentication](https://tools.ietf.org/html/rfc6750) |
| `digest` | `array` | The attributes for [Digest Authentication](https://en.wikipedia.org/wiki/Digest_access_authentication). |
| `edgegrid` | `array` | The attributes for [Akamai EdgeGrid Authentication](https://developer.akamai.com/legacy/introduction/Client_Auth.html). |
| `hawk` | `array` | The attributes for [Hawk Authentication](https://github.com/hueniverse/hawk) |
| `ntlm` | `array` | The attributes for [NTLM Authentication](https://msdn.microsoft.com/en-us/library/cc237488.aspx) |
| `oauth1` | `array` | The attributes for [OAuth2](https://oauth.net/1/) |
| `oauth2` | `array` | Helper attributes for [OAuth2](https://oauth.net/2/) |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "title": "Auth",
  "id": "#/definitions/auth",
  "description": "Represents authentication helpers provided by Postman",
  "properties": {
    "type": {
      "type": "string",
      "enum": [
        "apikey",
        "awsv4",
        "basic",
        "bearer",
        "digest",
        "edgegrid",
        "hawk",
        "noauth",
        "oauth1",
        "oauth2",
        "ntlm"
      ]
    },
    "noauth": {},
    "apikey": {
      "type": "array",
      "title": "API Key Authentication",
      "description": "The attributes for API Key Authentication.",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "awsv4": {
      "type": "array",
      "title": "AWS Signature v4",
      "description": "The attributes for [AWS Auth](http://docs.aws.amazon.com/AmazonS3/latest/dev/RESTAuthentication.html).",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "basic": {
      "type": "array",
      "title": "Basic Authentication",
      "description": "The attributes for [Basic Authentication](https://en.wikipedia.org/wiki/Basic_access_authentication).",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "bearer": {
      "type": "array",
      "title": "Bearer Token Authentication",
      "description": "The helper attributes for [Bearer Token Authentication](https://tools.ietf.org/html/rfc6750)",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "digest": {
      "type": "array",
      "title": "Digest Authentication",
      "description": "The attributes for [Digest Authentication](https://en.wikipedia.org/wiki/Digest_access_authentication).",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "edgegrid": {
      "type": "array",
      "title": "EdgeGrid Authentication",
      "description": "The attributes for [Akamai EdgeGrid Authentication](https://developer.akamai.com/legacy/introduction/Client_Auth.html).",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "hawk": {
      "type": "array",
      "title": "Hawk Authentication",
      "description": "The attributes for [Hawk Authentication](https://github.com/hueniverse/hawk)",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "ntlm": {
      "type": "array",
      "title": "NTLM Authentication",
      "description": "The attributes for [NTLM Authentication](https://msdn.microsoft.com/en-us/library/cc237488.aspx)",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "oauth1": {
      "type": "array",
      "title": "OAuth1",
      "description": "The attributes for [OAuth2](https://oauth.net/1/)",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    },
    "oauth2": {
      "type": "array",
      "title": "OAuth2",
      "description": "Helper attributes for [OAuth2](https://oauth.net/2/)",
      "items": {
        "$ref": "#/definitions/auth-attribute"
      }
    }
  },
  "required": [
    "type"
  ]
}
```

