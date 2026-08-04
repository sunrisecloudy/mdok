# T0139: GET method request 9

<!-- mdok-corpus id=T0139 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_8
curl --request GET "{{base_url}}/echo?case=method-8"
```

```jmespath mdok check=method_8
status == `200`
body.method == 'GET'
```
