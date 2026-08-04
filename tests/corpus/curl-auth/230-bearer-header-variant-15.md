# T0230: bearer header variant 15

<!-- mdok-corpus id=T0230 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_14
curl --header "Authorization: Bearer test-token" "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_14
status == `200`
body.authenticated == `true`
```
