# T0187: data raw json variant 17

<!-- mdok-corpus id=T0187 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_16
curl "{{base_url}}/echo" --header "Content-Type: application/json" --data-raw '{"a":1}'
```

```jmespath mdok check=body_16
status == `200`
body.json.a == `1`
```
