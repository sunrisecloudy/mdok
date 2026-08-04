# T0090: repeated headers variant 30

<!-- mdok-corpus id=T0090 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_29
curl "{{base_url}}/echo" -H "X-A: 1" -H "X-A: 2"
```

```jmespath mdok check=shell_29
status == `200`
```
