# T0188: json option variant 18

<!-- mdok-corpus id=T0188 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_17
curl "{{base_url}}/echo" --json '{"a":2}'
```

```jmespath mdok check=body_17
status == `200`
body.json.a == `2`
```
