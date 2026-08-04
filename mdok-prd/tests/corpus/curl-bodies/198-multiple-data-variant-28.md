# T0198: multiple data variant 28

<!-- mdok-corpus id=T0198 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_27
curl "{{base_url}}/echo" --data "a=1" --data "b=2"
```

```jmespath mdok check=body_27
status == `200`
contains(body.text, 'a=1')
```
