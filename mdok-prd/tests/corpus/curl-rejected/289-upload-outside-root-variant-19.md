# T0289: upload outside root variant 19

<!-- mdok-corpus id=T0289 category=curl-rejected stage=plan expected=error error=MDOK-E303 -->

```curl mdok name=rejected_18
curl --upload-file /etc/passwd "{{base_url}}/upload"
```
