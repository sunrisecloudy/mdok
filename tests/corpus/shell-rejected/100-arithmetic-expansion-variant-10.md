# T0100: arithmetic expansion variant 10

<!-- mdok-corpus id=T0100 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_9
curl "{{base_url}}/echo?n=$((1+1))"
```
