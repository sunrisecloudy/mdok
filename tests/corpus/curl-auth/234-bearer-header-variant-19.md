# T0234: bearer header variant 19

<!-- mdok-corpus id=T0234 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_18
curl --header "Authorization: Bearer test-token" "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_18
status == `200`
body.authenticated == `true`
```
