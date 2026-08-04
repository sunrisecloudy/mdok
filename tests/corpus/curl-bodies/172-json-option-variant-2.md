# T0172: json option variant 2

<!-- mdok-corpus id=T0172 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_1
curl "{{base_url}}/echo" --json '{"a":2}'
```

```jmespath mdok check=body_1
status == `200`
body.json.a == `2`
```
