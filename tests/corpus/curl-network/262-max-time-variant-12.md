# T0262: max time variant 12

<!-- mdok-corpus id=T0262 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_11
curl --max-time 5 "{{base_url}}/echo?case=11"
```

```jmespath mdok check=network_11
status == `200`
```
