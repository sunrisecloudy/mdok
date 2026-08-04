# T0258: user agent variant 8

<!-- mdok-corpus id=T0258 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_7
curl --user-agent mdok-test/1 "{{base_url}}/echo?case=7"
```

```jmespath mdok check=network_7
status == `200`
```
