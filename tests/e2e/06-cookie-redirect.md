# Cookie-aware redirect

The fixture follows one redirect and returns the cookie received by its final
endpoint. This keeps the workflow focused on redirect handling with an
explicit cookie header.

```curl mdok name=redirect_with_cookie
curl --location --max-redirs 3 \
  --cookie "fixture=ok" \
  "{{base_url}}/redirect/1?final=/cookies/echo"
```

```jmespath mdok check=redirect_with_cookie
status == `200`
transfer.redirect_count == `1`
body.cookies.fixture == 'ok'
```
