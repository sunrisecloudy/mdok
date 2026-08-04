# T0138: PROPFIND method request 8

<!-- mdok-corpus id=T0138 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_7
curl --request PROPFIND "{{base_url}}/echo?case=method-7"
```

```jmespath mdok check=method_7
status == `200`
body.method == 'PROPFIND'
```
