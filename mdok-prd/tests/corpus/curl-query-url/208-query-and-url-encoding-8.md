# T0208: query and url encoding 8

<!-- mdok-corpus id=T0208 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_7 = "space slash/plus+ไทย"
```

```curl mdok name=query_7
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_7|string}}" --data "n=7"
```

```jmespath mdok check=query_7
status == `200`
body.query.n == '7'
```
