# User CRUD flow

```curl mdok name=create
curl --request POST "{{base_url}}/users" \
  --header "Content-Type: application/json" \
  --data-raw '{"name":"Ada"}'
```

```jmespath mdok check=create
status == `201`
body.name == 'Ada'
```

```jmespath mdok capture=create
{user_id: body.id}
```

```curl mdok name=read
curl "{{base_url}}/users/{{user_id|url}}"
```

```jmespath mdok check=read
status == `200`
body.id == variables.user_id
```
