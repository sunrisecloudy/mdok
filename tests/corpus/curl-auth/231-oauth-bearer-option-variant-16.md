# T0231: oauth bearer option variant 16

<!-- mdok-corpus id=T0231 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_15
curl --oauth2-bearer test-token "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_15
status == `200`
body.authenticated == `true`
```
