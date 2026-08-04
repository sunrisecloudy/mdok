# T0288: header file outside root variant 18

<!-- mdok-corpus id=T0288 category=curl-rejected stage=plan expected=error error=MDOK-E303 -->

```curl mdok name=rejected_17
curl --header @/etc/passwd "{{base_url}}/echo"
```
