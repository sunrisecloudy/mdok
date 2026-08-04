# T0286: insecure denied variant 16

<!-- mdok-corpus id=T0286 category=curl-rejected stage=plan expected=error error=MDOK-E602 -->

```curl mdok name=rejected_15
curl --insecure "{{https_base_url}}/health"
```
