# T0272: next transfer variant 2

<!-- mdok-corpus id=T0272 category=curl-rejected stage=plan expected=error error=MDOK-E304 -->

```curl mdok name=rejected_1
curl "{{base_url}}/echo" --next "{{base_url}}/echo"
```
