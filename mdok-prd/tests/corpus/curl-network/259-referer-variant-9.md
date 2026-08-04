# T0259: referer variant 9

<!-- mdok-corpus id=T0259 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_8
curl --referer https://example.test/source "{{base_url}}/echo?case=8"
```

```jmespath mdok check=network_8
status == `200`
```
