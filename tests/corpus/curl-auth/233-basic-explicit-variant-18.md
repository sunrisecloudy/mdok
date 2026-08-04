# T0233: basic explicit variant 18

<!-- mdok-corpus id=T0233 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_17
curl --basic --user mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_17
status == `200`
body.authenticated == `true`
```
