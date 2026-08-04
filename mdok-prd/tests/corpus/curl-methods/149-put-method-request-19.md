# T0149: PUT method request 19

<!-- mdok-corpus id=T0149 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_18
curl --request PUT "{{base_url}}/echo?case=method-18"
```

```jmespath mdok check=method_18
status == `200`
body.method == 'PUT'
```
