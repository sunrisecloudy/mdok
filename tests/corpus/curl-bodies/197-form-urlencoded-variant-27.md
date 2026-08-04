# T0197: form urlencoded variant 27

<!-- mdok-corpus id=T0197 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_26
curl "{{base_url}}/echo" --data-urlencode "name=A B"
```

```jmespath mdok check=body_26
status == `200`
body.form.name == 'A B'
```
