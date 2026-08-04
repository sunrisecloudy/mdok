# T0251: connect timeout variant 1

<!-- mdok-corpus id=T0251 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_0
curl --connect-timeout 2 "{{base_url}}/echo?case=0"
```

```jmespath mdok check=network_0
status == `200`
```
