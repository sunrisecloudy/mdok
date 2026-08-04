# T0193: multipart field variant 23

<!-- mdok-corpus id=T0193 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_22
curl "{{base_url}}/multipart" --form "name=Ada"
```

```jmespath mdok check=body_22
status == `200`
body.multipart.fields.name == 'Ada'
```
