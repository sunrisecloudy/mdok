# Response

> Reference page for the `response` object of the Postman Collection Format v2.1.0.
> Generated from the canonical schema (see `../schemas/collection-v2.1.0.json`),
> matching the field-level reference tables once published at learning.postman.com/collection-format/reference/ (those pages are offline as of 2025; see README.md).

A response represents an HTTP response.

## Reference table

### Response

| Property | Type | Description |
| --- | --- | --- |
| `id` | `string` | A unique, user defined identifier that can  be used to refer to this response from requests. |
| `originalRequest` | `object | string` | A request represents an HTTP request. If a string, the string is assumed to be the request URL and the method is assumed to be 'GET'. |
| `responseTime` | `string | number | null` | The time taken by the request to complete. If a number, the unit is milliseconds. If the response is manually created, this can be set to `null`. |
| `timings` | `object | null` | Set of timing information related to request and response in milliseconds |
| `header` | `array | string | null` |  |
| `cookie` | `array` |  |
| `body` | `null/string` | The raw text of the response. |
| `status` | `string` | The response status, e.g: '200 OK' |
| `code` | `integer` | The numerical response code, example: 200, 201, 404, etc. |

## Schema

```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "id": "#/definitions/response",
  "title": "Response",
  "description": "A response represents an HTTP response.",
  "properties": {
    "id": {
      "description": "A unique, user defined identifier that can  be used to refer to this response from requests.",
      "type": "string"
    },
    "originalRequest": {
      "$ref": "#/definitions/request"
    },
    "responseTime": {
      "title": "ResponseTime",
      "oneOf": [
        {
          "type": "null"
        },
        {
          "type": "string"
        },
        {
          "type": "number"
        }
      ],
      "description": "The time taken by the request to complete. If a number, the unit is milliseconds. If the response is manually created, this can be set to `null`."
    },
    "timings": {
      "title": "Response Timings",
      "description": "Set of timing information related to request and response in milliseconds",
      "oneOf": [
        {
          "type": "object"
        },
        {
          "type": "null"
        }
      ]
    },
    "header": {
      "title": "Headers",
      "oneOf": [
        {
          "type": "array",
          "title": "Header",
          "description": "No HTTP request is complete without its headers, and the same is true for a Postman request. This field is an array containing all the headers.",
          "items": {
            "oneOf": [
              {
                "$ref": "#/definitions/header"
              },
              {
                "title": "Header",
                "type": "string"
              }
            ]
          }
        },
        {
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "cookie": {
      "type": "array",
      "items": {
        "$ref": "#/definitions/cookie"
      }
    },
    "body": {
      "type": [
        "null",
        "string"
      ],
      "description": "The raw text of the response."
    },
    "status": {
      "type": "string",
      "description": "The response status, e.g: '200 OK'"
    },
    "code": {
      "type": "integer",
      "description": "The numerical response code, example: 200, 201, 404, etc."
    }
  }
}
```

