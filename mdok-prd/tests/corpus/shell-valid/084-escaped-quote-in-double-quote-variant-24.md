# T0084: escaped quote in double quote variant 24

<!-- mdok-corpus id=T0084 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_23
curl "{{base_url}}/echo" --header "X-Test: a\"b"
```

```jmespath mdok check=shell_23
status == `200`
```
