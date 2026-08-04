# T0201: query and url encoding 1

<!-- mdok-corpus id=T0201 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_0 = "space slash/plus+ไทย"
```

```curl mdok name=query_0
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_0|string}}" --data "n=0"
```

```jmespath mdok check=query_0
status == `200`
body.query.n == '0'
```
