# T0088: long option equals variant 28

<!-- mdok-corpus id=T0088 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_27
curl --request=GET "{{base_url}}/echo"
```

```jmespath mdok check=shell_27
status == `200`
```
