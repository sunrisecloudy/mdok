# T0177: multipart field variant 7

<!-- mdok-corpus id=T0177 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_6
curl "{{base_url}}/multipart" --form "name=Ada"
```

```jmespath mdok check=body_6
status == `200`
body.multipart.fields.name == 'Ada'
```
