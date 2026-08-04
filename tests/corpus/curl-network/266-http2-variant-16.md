# T0266: http2 variant 16

<!-- mdok-corpus id=T0266 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_15
curl --http2 "{{base_url}}/echo?case=15"
```

```jmespath mdok check=network_15
status == `200`
```
