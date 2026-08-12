# Health status

The smallest executable workflow: request a health resource and assert both
the HTTP status and its JSON contract.

```curl mdok name=health
curl "{{base_url}}/health"
```

```jmespath mdok check=health
status == `200`
body.ok == `true`
```
