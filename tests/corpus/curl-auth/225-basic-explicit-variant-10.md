# T0225: basic explicit variant 10

<!-- mdok-corpus id=T0225 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_9
curl --basic --user mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_9
status == `200`
body.authenticated == `true`
```
