# T0179: data raw json variant 9

<!-- mdok-corpus id=T0179 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_8
curl "{{base_url}}/echo" --header "Content-Type: application/json" --data-raw '{"a":1}'
```

```jmespath mdok check=body_8
status == `200`
body.json.a == `1`
```
