# T0096: input redirection variant 6

<!-- mdok-corpus id=T0096 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_5
curl --data-binary @- "{{base_url}}/echo" < file
```
