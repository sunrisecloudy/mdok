# T0224: basic short variant 9

<!-- mdok-corpus id=T0224 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_8
curl -u mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_8
status == `200`
body.authenticated == `true`
```
