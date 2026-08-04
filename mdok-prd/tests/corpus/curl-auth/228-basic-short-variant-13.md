# T0228: basic short variant 13

<!-- mdok-corpus id=T0228 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_12
curl -u mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_12
status == `200`
body.authenticated == `true`
```
