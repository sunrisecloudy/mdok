# T0220: basic short variant 5

<!-- mdok-corpus id=T0220 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_4
curl -u mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_4
status == `200`
body.authenticated == `true`
```
