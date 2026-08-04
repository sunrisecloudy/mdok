# T0226: bearer header variant 11

<!-- mdok-corpus id=T0226 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_10
curl --header "Authorization: Bearer test-token" "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_10
status == `200`
body.authenticated == `true`
```
