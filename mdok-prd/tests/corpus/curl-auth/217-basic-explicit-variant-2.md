# T0217: basic explicit variant 2

<!-- mdok-corpus id=T0217 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_1
curl --basic --user mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_1
status == `200`
body.authenticated == `true`
```
