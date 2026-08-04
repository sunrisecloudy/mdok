# T0219: oauth bearer option variant 4

<!-- mdok-corpus id=T0219 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_3
curl --oauth2-bearer test-token "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_3
status == `200`
body.authenticated == `true`
```
