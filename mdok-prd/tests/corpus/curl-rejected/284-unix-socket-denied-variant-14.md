# T0284: unix socket denied variant 14

<!-- mdok-corpus id=T0284 category=curl-rejected stage=plan expected=error error=MDOK-E303 -->

```curl mdok name=rejected_13
curl --unix-socket /tmp/service.sock http://localhost/
```
