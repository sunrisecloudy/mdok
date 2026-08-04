# T0173: form urlencoded variant 3

<!-- mdok-corpus id=T0173 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_2
curl "{{base_url}}/echo" --data-urlencode "name=A B"
```

```jmespath mdok check=body_2
status == `200`
body.form.name == 'A B'
```
