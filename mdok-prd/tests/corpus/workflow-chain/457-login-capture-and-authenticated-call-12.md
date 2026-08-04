# T0457: login capture and authenticated call 12

<!-- mdok-corpus id=T0457 category=workflow-chain stage=execute expected=pass -->

```toml mdok vars
email = "agent11@example.com"
password = "test-password"
```

```curl mdok name=login_11
curl --request POST "{{base_url}}/auth/login" \
  --header "Content-Type: application/json" \
  --data-raw '{"email":{{email|json}},"password":{{password|json}}}'
```

```jmespath mdok check=login_11
status == `200`
body.user.email == variables.email
```

```jmespath mdok capture=login_11
{token: body.access_token, user_id: body.user.id}
```

```curl mdok name=profile_11
curl "{{base_url}}/users/{{user_id|url}}" --header "Authorization: Bearer {{token|header}}"
```

```jmespath mdok check=profile_11
status == `200`
body.id == variables.user_id
```
