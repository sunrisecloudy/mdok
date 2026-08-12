# Combined authentication and CRUD workflow

This deliberately combines the features that an agent commonly needs when
designing an API: setup, authentication, captures, JSON bodies, headers,
ordered dependent requests, assertions, and cleanup. The fixture-only values
below are safe test data; a real workflow should receive credentials through
CLI/MCP inputs or an explicit environment file.

```toml mdok vars
email = "combined-e2e@example.com"
fixture_password = "test-password"
```

```curl mdok name=health
curl "{{base_url}}/health"
```

```jmespath mdok check=health
status == `200`
body.ok == `true`
```

```curl mdok name=login
curl --request POST "{{base_url}}/auth/login" \
  --header "Content-Type: application/json" \
  --data-raw '{"email":{{email|json}},"password":{{fixture_password|json}}}'
```

```jmespath mdok check=login
status == `200`
body.user.email == variables.email
type(body.access_token) == 'string'
```

```jmespath mdok capture=login
{access_token: body.access_token}
```

```curl mdok name=create_user
curl --request POST "{{base_url}}/users" \
  --header "Content-Type: application/json" \
  --header "X-Mdok-Test-Key: combined-workflow" \
  --data-raw '{"id":"combined-e2e-user","name":"Ada","email":{{email|json}}}'
```

```jmespath mdok check=create_user
status == `201`
body.id == 'combined-e2e-user'
body.email == variables.email
```

```jmespath mdok capture=create_user
{resource_id: body.id}
```

```curl mdok name=read_user
curl "{{base_url}}/users/{{resource_id|url}}" \
  --header "X-Mdok-Test-Key: combined-workflow" \
  --header "Authorization: Bearer {{access_token|header}}"
```

```jmespath mdok check=read_user
status == `200`
body.id == variables.resource_id
body.name == 'Ada'
```

```curl mdok name=update_user
curl --request PATCH "{{base_url}}/users/{{resource_id|url}}" \
  --header "Content-Type: application/json" \
  --header "X-Mdok-Test-Key: combined-workflow" \
  --data-raw '{"name":"Ada Lovelace"}'
```

```jmespath mdok check=update_user
status == `200`
body.id == variables.resource_id
body.name == 'Ada Lovelace'
```

```curl mdok name=delete_user
curl --request DELETE "{{base_url}}/users/{{resource_id|url}}" \
  --header "X-Mdok-Test-Key: combined-workflow"
```

```jmespath mdok check=delete_user
status == `200`
body.deleted == `true`
body.id == variables.resource_id
```
