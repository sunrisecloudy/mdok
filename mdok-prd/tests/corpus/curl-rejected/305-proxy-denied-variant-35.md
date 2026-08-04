# T0305: proxy denied variant 35

<!-- mdok-corpus id=T0305 category=curl-rejected stage=plan expected=error error=MDOK-E604 -->

```curl mdok name=rejected_34
curl --proxy http://127.0.0.1:9999 "{{base_url}}/echo"
```
