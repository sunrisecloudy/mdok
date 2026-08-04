# T0147: GET method request 17

<!-- mdok-corpus id=T0147 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_16
curl --request GET "{{base_url}}/echo?case=method-16"
```

```jmespath mdok check=method_16
status == `200`
body.method == 'GET'
```
