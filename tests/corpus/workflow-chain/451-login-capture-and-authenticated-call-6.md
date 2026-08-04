# T0451: login capture and authenticated call 6

<!-- mdok-corpus id=T0451 category=workflow-chain stage=execute expected=pass -->

```toml mdok vars
email = "agent5@example.com"
password = "test-password"
```

```curl mdok name=login_5
curl --request POST "{{base_url}}/auth/login" \
  --header "Content-Type: application/json" \
  --data-raw '{"email":{{email|json}},"password":{{password|json}}}'
```

```jmespath mdok check=login_5
status == `200`
body.user.email == variables.email
```

```jmespath mdok capture=login_5
{token: body.access_token, user_id: body.user.id}
```

```curl mdok name=profile_5
curl "{{base_url}}/users/{{user_id|url}}" --header "Authorization: Bearer {{token|header}}"
```

```jmespath mdok check=profile_5
status == `200`
body.id == variables.user_id
```
