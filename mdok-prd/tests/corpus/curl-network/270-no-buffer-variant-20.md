# T0270: no buffer variant 20

<!-- mdok-corpus id=T0270 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_19
curl --no-buffer "{{base_url}}/echo?case=19"
```

```jmespath mdok check=network_19
status == `200`
```
