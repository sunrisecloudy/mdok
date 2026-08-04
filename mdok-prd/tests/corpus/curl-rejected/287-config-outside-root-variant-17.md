# T0287: config outside root variant 17

<!-- mdok-corpus id=T0287 category=curl-rejected stage=plan expected=error error=MDOK-E303 -->

```curl mdok name=rejected_16
curl --config /etc/curlrc "{{base_url}}/echo"
```
