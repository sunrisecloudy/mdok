# T0142: PATCH method request 12

<!-- mdok-corpus id=T0142 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_11
curl --request PATCH "{{base_url}}/echo?case=method-11"
```

```jmespath mdok check=method_11
status == `200`
body.method == 'PATCH'
```
