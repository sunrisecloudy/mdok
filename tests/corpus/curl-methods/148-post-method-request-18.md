# T0148: POST method request 18

<!-- mdok-corpus id=T0148 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_17
curl --request POST "{{base_url}}/echo?case=method-17"
```

```jmespath mdok check=method_17
status == `200`
body.method == 'POST'
```
