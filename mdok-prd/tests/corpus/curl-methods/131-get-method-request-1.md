# T0131: GET method request 1

<!-- mdok-corpus id=T0131 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_0
curl --request GET "{{base_url}}/echo?case=method-0"
```

```jmespath mdok check=method_0
status == `200`
body.method == 'GET'
```
