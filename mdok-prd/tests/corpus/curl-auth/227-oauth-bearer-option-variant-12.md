# T0227: oauth bearer option variant 12

<!-- mdok-corpus id=T0227 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_11
curl --oauth2-bearer test-token "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_11
status == `200`
body.authenticated == `true`
```
