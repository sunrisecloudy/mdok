# T0120: arithmetic expansion variant 30

<!-- mdok-corpus id=T0120 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_29
curl "{{base_url}}/echo?n=$((1+1))"
```
