# T0255: http1.1 variant 5

<!-- mdok-corpus id=T0255 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_4
curl --http1.1 "{{base_url}}/echo?case=4"
```

```jmespath mdok check=network_4
status == `200`
```
