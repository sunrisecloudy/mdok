# T0136: OPTIONS method request 6

<!-- mdok-corpus id=T0136 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_5
curl --request OPTIONS "{{base_url}}/echo?case=method-5"
```

```jmespath mdok check=method_5
status == `200`
body.method == 'OPTIONS'
```
