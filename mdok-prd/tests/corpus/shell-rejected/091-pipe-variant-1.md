# T0091: pipe variant 1

<!-- mdok-corpus id=T0091 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_0
curl "{{base_url}}/echo" | jq .
```
