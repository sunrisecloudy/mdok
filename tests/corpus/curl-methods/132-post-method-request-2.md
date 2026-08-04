# T0132: POST method request 2

<!-- mdok-corpus id=T0132 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_1
curl --request POST "{{base_url}}/echo?case=method-1"
```

```jmespath mdok check=method_1
status == `200`
body.method == 'POST'
```
