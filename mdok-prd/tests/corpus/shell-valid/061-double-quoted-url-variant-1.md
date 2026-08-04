# T0061: double quoted url variant 1

<!-- mdok-corpus id=T0061 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_0
curl "{{base_url}}/echo"
```

```jmespath mdok check=shell_0
status == `200`
```
