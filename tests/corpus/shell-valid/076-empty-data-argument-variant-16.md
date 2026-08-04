# T0076: empty data argument variant 16

<!-- mdok-corpus id=T0076 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_15
curl "{{base_url}}/echo" --data-raw ""
```

```jmespath mdok check=shell_15
status == `200`
```
