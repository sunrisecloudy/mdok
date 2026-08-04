# T0216: basic short variant 1

<!-- mdok-corpus id=T0216 category=curl-auth stage=execute expected=pass -->

```curl mdok name=auth_0
curl -u mdok:secret "{{base_url}}/auth/basic"
```

```jmespath mdok check=auth_0
status == `200`
body.authenticated == `true`
```
