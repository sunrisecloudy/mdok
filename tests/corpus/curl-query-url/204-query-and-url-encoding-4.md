# T0204: query and url encoding 4

<!-- mdok-corpus id=T0204 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_3 = "space slash/plus+ไทย"
```

```curl mdok name=query_3
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_3|string}}" --data "n=3"
```

```jmespath mdok check=query_3
status == `200`
body.query.n == '3'
```
