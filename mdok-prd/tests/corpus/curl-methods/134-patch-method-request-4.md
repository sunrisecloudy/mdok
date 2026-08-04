# T0134: PATCH method request 4

<!-- mdok-corpus id=T0134 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_3
curl --request PATCH "{{base_url}}/echo?case=method-3"
```

```jmespath mdok check=method_3
status == `200`
body.method == 'PATCH'
```
