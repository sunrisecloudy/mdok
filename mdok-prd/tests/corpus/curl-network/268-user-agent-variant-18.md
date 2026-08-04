# T0268: user agent variant 18

<!-- mdok-corpus id=T0268 category=curl-network stage=execute expected=pass -->

```curl mdok name=network_17
curl --user-agent mdok-test/1 "{{base_url}}/echo?case=17"
```

```jmespath mdok check=network_17
status == `200`
```
