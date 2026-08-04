# T0079: short option attached variant 19

<!-- mdok-corpus id=T0079 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_18
curl -XGET "{{base_url}}/echo"
```

```jmespath mdok check=shell_18
status == `200`
```
