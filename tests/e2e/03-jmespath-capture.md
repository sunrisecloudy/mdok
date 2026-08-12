# JMESPath capture

Captures turn a response value into an input for a later request. The fixture
returns a deterministic JSON collection, so this test has no external data or
credentials.

```curl mdok name=source
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=source
status == `200`
length(body.items) == `3`
```

```jmespath mdok capture=source
{captured_id: body.items[1].id}
```

```curl mdok name=use_capture
curl "{{base_url}}/echo?captured={{captured_id|url}}"
```

```jmespath mdok check=use_capture
status == `200`
body.query.captured == variables.captured_id
```
