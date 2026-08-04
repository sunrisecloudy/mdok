# T0069: short option attached variant 9

<!-- mdok-corpus id=T0069 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_8
curl -XGET "{{base_url}}/echo"
```

```jmespath mdok check=shell_8
status == `200`
```
