# Bearer authentication

`test-token` is a fixture-only credential implemented by `mdok-test-server`; it
is not a production secret. This file focuses on sending an Authorization
header and checking the authenticated response.

```toml mdok vars
fixture_credential = "test-token"
```

```curl mdok name=bearer
curl "{{base_url}}/auth/bearer" \
  --header "Authorization: Bearer {{fixture_credential|header}}"
```

```jmespath mdok check=bearer
status == `200`
body.authenticated == `true`
body.ok == `true`
```
