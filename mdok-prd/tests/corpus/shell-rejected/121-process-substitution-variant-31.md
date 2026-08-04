# T0121: process substitution variant 31

<!-- mdok-corpus id=T0121 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_30
curl --data-binary @<(echo x) "{{base_url}}/echo"
```
