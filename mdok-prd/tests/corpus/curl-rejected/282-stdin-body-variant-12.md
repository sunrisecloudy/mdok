# T0282: stdin body variant 12

<!-- mdok-corpus id=T0282 category=curl-rejected stage=plan expected=error error=MDOK-E301 -->

```curl mdok name=rejected_11
curl --data-binary @- "{{base_url}}/echo"
```
