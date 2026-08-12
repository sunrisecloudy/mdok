# Template and query encoding

This workflow exercises a Markdown variable in a URL query and verifies that
MDOK preserves spaces, slashes, plus signs, and Unicode through URL encoding.

```toml mdok vars
query_value = "space slash/plus+ไทย"
```

```curl mdok name=query
curl --get "{{base_url}}/echo" \
  --data-urlencode "q={{query_value|string}}" \
  --data "page=2"
```

```jmespath mdok check=query
status == `200`
body.query.q == variables.query_value
body.query.page == '2'
```
