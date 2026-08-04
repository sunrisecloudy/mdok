# T0182: multiple data variant 12

<!-- mdok-corpus id=T0182 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_11
curl "{{base_url}}/echo" --data "a=1" --data "b=2"
```

```jmespath mdok check=body_11
status == `200`
contains(body.text, 'a=1')
```
