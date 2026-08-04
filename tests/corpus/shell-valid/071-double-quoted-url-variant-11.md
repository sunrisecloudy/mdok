# T0071: double quoted url variant 11

<!-- mdok-corpus id=T0071 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_10
curl "{{base_url}}/echo"
```

```jmespath mdok check=shell_10
status == `200`
```
