# T0304: unix socket denied variant 34

<!-- mdok-corpus id=T0304 category=curl-rejected stage=plan expected=error error=MDOK-E303 -->

```curl mdok name=rejected_33
curl --unix-socket /tmp/service.sock http://localhost/
```
