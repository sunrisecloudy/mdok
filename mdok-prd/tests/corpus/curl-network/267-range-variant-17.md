# T0267: range variant 17

<!-- mdok-corpus id=T0267 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_16
curl --range 0-9 "{{base_url}}/echo?case=16"
```

```jmespath mdok check=network_16
status == `200`
```
