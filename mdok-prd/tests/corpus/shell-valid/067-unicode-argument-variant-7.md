# T0067: unicode argument variant 7

<!-- mdok-corpus id=T0067 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_6
curl "{{base_url}}/echo" --header "X-Name: สวัสดี"
```

```jmespath mdok check=shell_6
status == `200`
```
