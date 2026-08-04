# T0302: stdin body variant 32

<!-- mdok-corpus id=T0302 category=curl-rejected stage=plan expected=error error=MDOK-E301 -->

```curl mdok name=rejected_31
curl --data-binary @- "{{base_url}}/echo"
```
