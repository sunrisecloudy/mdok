# T0089: short option attached variant 29

<!-- mdok-corpus id=T0089 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_28
curl -XGET "{{base_url}}/echo"
```

```jmespath mdok check=shell_28
status == `200`
```
