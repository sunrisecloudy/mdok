# T0068: long option equals variant 8

<!-- mdok-corpus id=T0068 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_7
curl --request=GET "{{base_url}}/echo"
```

```jmespath mdok check=shell_7
status == `200`
```
