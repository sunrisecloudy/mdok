# T0141: PUT method request 11

<!-- mdok-corpus id=T0141 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_10
curl --request PUT "{{base_url}}/echo?case=method-10"
```

```jmespath mdok check=method_10
status == `200`
body.method == 'PUT'
```
