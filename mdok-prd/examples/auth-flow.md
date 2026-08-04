# Authentication flow

```toml mdok vars
base_url = "http://127.0.0.1:9800"
email = "agent@example.com"
password = "test-password"
```

```curl mdok name=login
curl --request POST "{{base_url}}/auth/login" \
  --header "Content-Type: application/json" \
  --data-raw '{"email":{{email|json}},"password":{{password|json}}}'
```

```jmespath mdok check=login
status == `200`
body.user.email == variables.email
type(body.access_token) == 'string'
```

```jmespath mdok capture=login
{access_token: body.access_token, user_id: body.user.id}
```

```curl mdok name=get_profile
curl "{{base_url}}/users/{{user_id|url}}" \
  --header "Authorization: Bearer {{access_token|header}}"
```

```jmespath mdok check=get_profile
status == `200`
body.id == variables.user_id
```
