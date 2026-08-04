# T0212: query and url encoding 12

<!-- mdok-corpus id=T0212 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_11 = "space slash/plus+ไทย"
```

```curl mdok name=query_11
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_11|string}}" --data "n=11"
```

```jmespath mdok check=query_11
status == `200`
body.query.n == '11'
```
