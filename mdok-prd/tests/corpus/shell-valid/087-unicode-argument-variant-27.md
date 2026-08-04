# T0087: unicode argument variant 27

<!-- mdok-corpus id=T0087 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_26
curl "{{base_url}}/echo" --header "X-Name: สวัสดี"
```

```jmespath mdok check=shell_26
status == `200`
```
