# T0263: retry variant 13

<!-- mdok-corpus id=T0263 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_12
curl --retry 2 --retry-delay 0 "{{base_url}}/echo?case=12"
```

```jmespath mdok check=network_12
status == `200`
```
