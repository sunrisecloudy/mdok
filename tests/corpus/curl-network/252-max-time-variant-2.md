# T0252: max time variant 2

<!-- mdok-corpus id=T0252 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_1
curl --max-time 5 "{{base_url}}/echo?case=1"
```

```jmespath mdok check=network_1
status == `200`
```
