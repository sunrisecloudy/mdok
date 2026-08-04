# T0135: DELETE method request 5

<!-- mdok-corpus id=T0135 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_4
curl --request DELETE "{{base_url}}/echo?case=method-4"
```

```jmespath mdok check=method_4
status == `200`
body.method == 'DELETE'
```
