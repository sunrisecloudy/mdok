# T0077: unicode argument variant 17

<!-- mdok-corpus id=T0077 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_16
curl "{{base_url}}/echo" --header "X-Name: สวัสดี"
```

```jmespath mdok check=shell_16
status == `200`
```
