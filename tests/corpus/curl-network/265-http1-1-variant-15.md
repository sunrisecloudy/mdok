# T0265: http1.1 variant 15

<!-- mdok-corpus id=T0265 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_14
curl --http1.1 "{{base_url}}/echo?case=14"
```

```jmespath mdok check=network_14
status == `200`
```
