# T0215: query and url encoding 15

<!-- mdok-corpus id=T0215 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_14 = "space slash/plus+ไทย"
```

```curl mdok name=query_14
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_14|string}}" --data "n=14"
```

```jmespath mdok check=query_14
status == `200`
body.query.n == '14'
```
