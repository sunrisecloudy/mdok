# T0143: DELETE method request 13

<!-- mdok-corpus id=T0143 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_12
curl --request DELETE "{{base_url}}/echo?case=method-12"
```

```jmespath mdok check=method_12
status == `200`
body.method == 'DELETE'
```
