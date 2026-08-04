# T0260: no buffer variant 10

<!-- mdok-corpus id=T0260 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_9
curl --no-buffer "{{base_url}}/echo?case=9"
```

```jmespath mdok check=network_9
status == `200`
```
