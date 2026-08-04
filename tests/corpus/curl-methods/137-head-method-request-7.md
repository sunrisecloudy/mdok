# T0137: HEAD method request 7

<!-- mdok-corpus id=T0137 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_6
curl --request HEAD "{{base_url}}/echo?case=method-6"
```

```jmespath mdok check=method_6
status == `200`
body.method == 'HEAD'
```
