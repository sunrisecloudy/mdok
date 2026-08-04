# T0269: referer variant 19

<!-- mdok-corpus id=T0269 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_18
curl --referer https://example.test/source "{{base_url}}/echo?case=18"
```

```jmespath mdok check=network_18
status == `200`
```
