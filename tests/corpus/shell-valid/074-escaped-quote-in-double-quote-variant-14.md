# T0074: escaped quote in double quote variant 14

<!-- mdok-corpus id=T0074 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_13
curl "{{base_url}}/echo" --header "X-Test: a\"b"
```

```jmespath mdok check=shell_13
status == `200`
```
