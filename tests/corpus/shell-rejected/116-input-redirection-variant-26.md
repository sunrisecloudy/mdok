# T0116: input redirection variant 26

<!-- mdok-corpus id=T0116 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_25
curl --data-binary @- "{{base_url}}/echo" < file
```
