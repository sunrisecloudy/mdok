# T0232: basic short variant 17

<!-- mdok-corpus id=T0232 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_16
curl -u mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_16
status == `200`
body.authenticated == `true`
```
