# T0140: POST method request 10

<!-- mdok-corpus id=T0140 category=curl-methods stage=execute expected=pass -->

```curl mdok name=method_9
curl --request POST "{{base_url}}/echo?case=method-9"
```

```jmespath mdok check=method_9
status == `200`
body.method == 'POST'
```
