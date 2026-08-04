# T0064: escaped quote in double quote variant 4

<!-- mdok-corpus id=T0064 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_3
curl "{{base_url}}/echo" --header "X-Test: a\"b"
```

```jmespath mdok check=shell_3
status == `200`
```
