# T0285: proxy denied variant 15

<!-- mdok-corpus id=T0285 category=curl-rejected stage=plan expected=error error=MDOK-E604 -->

```curl mdok name=rejected_14
curl --proxy http://127.0.0.1:9999 "{{base_url}}/echo"
```
