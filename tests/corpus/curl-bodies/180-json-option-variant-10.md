# T0180: json option variant 10

<!-- mdok-corpus id=T0180 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_9
curl "{{base_url}}/echo" --json '{"a":2}'
```

```jmespath mdok check=body_9
status == `200`
body.json.a == `2`
```
