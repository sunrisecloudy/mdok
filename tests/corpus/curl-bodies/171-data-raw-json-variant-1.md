# T0171: data raw json variant 1

<!-- mdok-corpus id=T0171 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_0
curl "{{base_url}}/echo" --header "Content-Type: application/json" --data-raw '{"a":1}'
```

```jmespath mdok check=body_0
status == `200`
body.json.a == `1`
```
