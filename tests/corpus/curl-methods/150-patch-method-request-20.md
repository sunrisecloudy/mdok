# T0150: PATCH method request 20

<!-- mdok-corpus id=T0150 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_19
curl --request PATCH "{{base_url}}/echo?case=method-19"
```

```jmespath mdok check=method_19
status == `200`
body.method == 'PATCH'
```
