# T0257: range variant 7

<!-- mdok-corpus id=T0257 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_6
curl --range 0-9 "{{base_url}}/echo?case=6"
```

```jmespath mdok check=network_6
status == `200`
```
