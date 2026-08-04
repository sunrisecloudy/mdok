# T0190: multiple data variant 20

<!-- mdok-corpus id=T0190 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_19
curl "{{base_url}}/echo" --data "a=1" --data "b=2"
```

```jmespath mdok check=body_19
status == `200`
contains(body.text, 'a=1')
```
