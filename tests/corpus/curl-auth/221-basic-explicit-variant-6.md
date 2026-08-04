# T0221: basic explicit variant 6

<!-- mdok-corpus id=T0221 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_5
curl --basic --user mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_5
status == `200`
body.authenticated == `true`
```
