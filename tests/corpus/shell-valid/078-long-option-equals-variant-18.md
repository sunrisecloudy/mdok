# T0078: long option equals variant 18

<!-- mdok-corpus id=T0078 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_17
curl --request=GET "{{base_url}}/echo"
```

```jmespath mdok check=shell_17
status == `200`
```
