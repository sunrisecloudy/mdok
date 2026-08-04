# T0196: json option variant 26

<!-- mdok-corpus id=T0196 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_25
curl "{{base_url}}/echo" --json '{"a":2}'
```

```jmespath mdok check=body_25
status == `200`
body.json.a == `2`
```
