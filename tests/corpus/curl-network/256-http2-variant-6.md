# T0256: http2 variant 6

<!-- mdok-corpus id=T0256 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_5
curl --http2 "{{base_url}}/echo?case=5"
```

```jmespath mdok check=network_5
status == `200`
```
