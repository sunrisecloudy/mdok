# T0174: multiple data variant 4

<!-- mdok-corpus id=T0174 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_3
curl "{{base_url}}/echo" --data "a=1" --data "b=2"
```

```jmespath mdok check=body_3
status == `200`
contains(body.text, 'a=1')
```
