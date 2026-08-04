# T0066: empty data argument variant 6

<!-- mdok-corpus id=T0066 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_5
curl "{{base_url}}/echo" --data-raw ""
```

```jmespath mdok check=shell_5
status == `200`
```
