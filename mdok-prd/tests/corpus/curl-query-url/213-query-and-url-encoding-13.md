# T0213: query and url encoding 13

<!-- mdok-corpus id=T0213 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_12 = "space slash/plus+ไทย"
```

```curl mdok name=query_12
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_12|string}}" --data "n=12"
```

```jmespath mdok check=query_12
status == `200`
body.query.n == '12'
```
