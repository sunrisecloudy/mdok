# T0080: repeated headers variant 20

<!-- mdok-corpus id=T0080 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_19
curl "{{base_url}}/echo" -H "X-A: 1" -H "X-A: 2"
```

```jmespath mdok check=shell_19
status == `200`
```
