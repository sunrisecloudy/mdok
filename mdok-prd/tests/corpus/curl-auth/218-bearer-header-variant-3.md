# T0218: bearer header variant 3

<!-- mdok-corpus id=T0218 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_2
curl --header "Authorization: Bearer test-token" "{{base_url}}/auth/bearer"
```

```jmespath mdok check=auth_2
status == `200`
body.authenticated == `true`
```
