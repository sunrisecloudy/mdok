# T0181: form urlencoded variant 11

<!-- mdok-corpus id=T0181 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_10
curl "{{base_url}}/echo" --data-urlencode "name=A B"
```

```jmespath mdok check=body_10
status == `200`
body.form.name == 'A B'
```
