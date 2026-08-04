# T0202: query and url encoding 2

<!-- mdok-corpus id=T0202 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_1 = "space slash/plus+ไทย"
```

```curl mdok name=query_1
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_1|string}}" --data "n=1"
```

```jmespath mdok check=query_1
status == `200`
body.query.n == '1'
```
