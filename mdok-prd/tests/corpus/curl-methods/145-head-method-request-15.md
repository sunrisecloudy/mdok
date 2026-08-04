# T0145: HEAD method request 15

<!-- mdok-corpus id=T0145 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_14
curl --request HEAD "{{base_url}}/echo?case=method-14"
```

```jmespath mdok check=method_14
status == `200`
body.method == 'HEAD'
```
