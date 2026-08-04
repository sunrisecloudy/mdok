# T0133: PUT method request 3

<!-- mdok-corpus id=T0133 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_2
curl --request PUT "{{base_url}}/echo?case=method-2"
```

```jmespath mdok check=method_2
status == `200`
body.method == 'PUT'
```
