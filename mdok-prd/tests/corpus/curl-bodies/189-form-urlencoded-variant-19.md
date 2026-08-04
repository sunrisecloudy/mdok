# T0189: form urlencoded variant 19

<!-- mdok-corpus id=T0189 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_18
curl "{{base_url}}/echo" --data-urlencode "name=A B"
```

```jmespath mdok check=body_18
status == `200`
body.form.name == 'A B'
```
