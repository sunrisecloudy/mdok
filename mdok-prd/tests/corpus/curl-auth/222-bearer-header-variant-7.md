# T0222: bearer header variant 7

<!-- mdok-corpus id=T0222 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_6
curl --header "Authorization: Bearer test-token" "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_6
status == `200`
body.authenticated == `true`
```
