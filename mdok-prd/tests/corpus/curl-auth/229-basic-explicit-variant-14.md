# T0229: basic explicit variant 14

<!-- mdok-corpus id=T0229 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_13
curl --basic --user mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_13
status == `200`
body.authenticated == `true`
```
