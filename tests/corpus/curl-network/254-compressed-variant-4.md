# T0254: compressed variant 4

<!-- mdok-corpus id=T0254 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_3
curl --compressed "{{base_url}}/gzip?case=3"
```

```jmespath mdok check=network_3
status == `200`
```
