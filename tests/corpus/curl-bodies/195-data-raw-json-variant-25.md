# T0195: data raw json variant 25

<!-- mdok-corpus id=T0195 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_24
curl "{{base_url}}/echo" --header "Content-Type: application/json" --data-raw '{"a":1}'
```

```jmespath mdok check=body_24
status == `200`
body.json.a == `1`
```
