# T0211: query and url encoding 11

<!-- mdok-corpus id=T0211 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_10 = "space slash/plus+ไทย"
```

```curl mdok name=query_10
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_10|string}}" --data "n=10"
```

```jmespath mdok check=query_10
status == `200`
body.query.n == '10'
```
