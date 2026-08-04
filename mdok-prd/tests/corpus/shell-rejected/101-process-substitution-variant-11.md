# T0101: process substitution variant 11

<!-- mdok-corpus id=T0101 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_10
curl --data-binary @<(echo x) "{{base_url}}/echo"
```
