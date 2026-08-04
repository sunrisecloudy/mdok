# T0081: double quoted url variant 21

<!-- mdok-corpus id=T0081 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_20
curl "{{base_url}}/echo"
```

```jmespath mdok check=shell_20
status == `200`
```
