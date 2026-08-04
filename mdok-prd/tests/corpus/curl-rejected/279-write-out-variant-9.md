# T0279: write out variant 9

<!-- mdok-corpus id=T0279 category=curl-rejected stage=plan expected=error error=MDOK-E301 -->

```curl mdok name=rejected_8
curl --write-out "%{http_code}" "{{base_url}}/echo"
```
