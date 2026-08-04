# T0235: oauth bearer option variant 20

<!-- mdok-corpus id=T0235 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_19
curl --oauth2-bearer test-token "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_19
status == `200`
body.authenticated == `true`
```
