# T0144: OPTIONS method request 14

<!-- mdok-corpus id=T0144 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_13
curl --request OPTIONS "{{base_url}}/echo?case=method-13"
```

```jmespath mdok check=method_13
status == `200`
body.method == 'OPTIONS'
```
