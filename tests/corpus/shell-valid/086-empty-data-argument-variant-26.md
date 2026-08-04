# T0086: empty data argument variant 26

<!-- mdok-corpus id=T0086 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_25
curl "{{base_url}}/echo" --data-raw ""
```

```jmespath mdok check=shell_25
status == `200`
```
