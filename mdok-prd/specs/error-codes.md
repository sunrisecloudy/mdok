# Error Code Registry

| Code | Meaning |
|---|---|
| MDOK-E001 | Invalid UTF-8 or document input. |
| MDOK-E100 | Invalid executable fence metadata. |
| MDOK-E101 | Duplicate or invalid step name. |
| MDOK-E102 | Unknown step reference or invalid order. |
| MDOK-E110 | Invalid TOML variables block. |
| MDOK-E200 | Invalid shell/Bash syntax in curl fence. |
| MDOK-E201 | Forbidden shell construct. |
| MDOK-E202 | Command is not exactly one `curl` simple command. |
| MDOK-E300 | curl option parse failure. |
| MDOK-E301 | curl option unsupported by MDOK policy. |
| MDOK-E302 | URL/protocol denied. |
| MDOK-E303 | Filesystem access denied. |
| MDOK-E304 | Multiple curl transfers are not supported in version 1. |
| MDOK-E305 | Required curl/libcurl build feature unavailable. |
| MDOK-E306 | External command is not allowed by the command policy. |
| MDOK-E307 | External command argv or direct-command syntax is invalid. |
| MDOK-E308 | External command could not be started or reaped. |
| MDOK-E309 | External command exited unsuccessfully. |
| MDOK-E310 | External command exceeded its timeout. |
| MDOK-E311 | External command argv or output resource limit exceeded. |
| MDOK-E312 | External command environment or working-directory policy violation. |
| MDOK-E400 | Invalid template syntax. |
| MDOK-E401 | Missing variable. |
| MDOK-E402 | Template type/filter error. |
| MDOK-E403 | Unsafe header value. |
| MDOK-E404 | Secret exposure policy violation. |
| MDOK-E500 | Invalid JMESPath syntax. |
| MDOK-E501 | JMESPath runtime or result type error. |
| MDOK-E502 | JMESPath check evaluated to false. |
| MDOK-E503 | Capture did not evaluate to an object. |
| MDOK-E504 | Capture key collision or invalid key. |
| MDOK-E600 | Transfer failure. |
| MDOK-E601 | Timeout or low-speed abort. |
| MDOK-E602 | TLS verification/configuration failure. |
| MDOK-E603 | Redirect policy failure. |
| MDOK-E604 | Proxy/DNS/connect policy failure. |
| MDOK-E610 | Response body parse failure under required JSON policy. |
| MDOK-E700 | Resource limit exceeded. |
| MDOK-E701 | Execution cancelled. |
| MDOK-E800 | Report write/serialization failure. |
| MDOK-E900 | Internal invariant or FFI error. |
