# T0223: oauth bearer option variant 8

<!-- mdok-corpus id=T0223 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_7
curl --oauth2-bearer test-token "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_7
status == `200`
body.authenticated == `true`
```
