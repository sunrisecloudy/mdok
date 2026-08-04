# T0146: PROPFIND method request 16

<!-- mdok-corpus id=T0146 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_15
curl --request PROPFIND "{{base_url}}/echo?case=method-15"
```

```jmespath mdok check=method_15
status == `200`
body.method == 'PROPFIND'
```
