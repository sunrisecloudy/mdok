# T0261: connect timeout variant 11

<!-- mdok-corpus id=T0261 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_10
curl --connect-timeout 2 "{{base_url}}/echo?case=10"
```

```jmespath mdok check=network_10
status == `200`
```
