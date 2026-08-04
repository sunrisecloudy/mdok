# T0253: retry variant 3

<!-- mdok-corpus id=T0253 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_2
curl --retry 2 --retry-delay 0 "{{base_url}}/echo?case=2"
```

```jmespath mdok check=network_2
status == `200`
```
