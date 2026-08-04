# T0185: multipart field variant 15

<!-- mdok-corpus id=T0185 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_14
curl "{{base_url}}/multipart" --form "name=Ada"
```

```jmespath mdok check=body_14
status == `200`
body.multipart.fields.name == 'Ada'
```
