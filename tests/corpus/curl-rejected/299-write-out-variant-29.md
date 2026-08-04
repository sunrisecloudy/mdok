# T0299: write out variant 29

<!-- mdok-corpus id=T0299 category=curl-rejected stage=plan expected=error error=MDOK-E301 -->

```curl mdok name=rejected_28
curl --write-out "%{http_code}" "{{base_url}}/echo"
```
