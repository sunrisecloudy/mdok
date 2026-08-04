# T0264: compressed variant 14

<!-- mdok-corpus id=T0264 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_13
curl --compressed "{{base_url}}/gzip?case=13"
```

```jmespath mdok check=network_13
status == `200`
```
