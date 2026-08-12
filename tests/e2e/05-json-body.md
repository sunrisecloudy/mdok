# JSON request body

This workflow focuses on JSON serialization in a request body and the typed
JSON response exposed by the echo fixture.

```toml mdok vars
payload_name = "Ada"
```

```curl mdok name=json_body
curl --request POST "{{base_url}}/echo" \
  --header "Content-Type: application/json" \
  --data-raw '{"name":{{payload_name|json}},"active":true}'
```

```jmespath mdok check=json_body
status == `200`
body.method == 'POST'
body.json.name == variables.payload_name
body.json.active == `true`
```
