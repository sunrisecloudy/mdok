# T0210: query and url encoding 10

<!-- mdok-corpus id=T0210 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_9 = "space slash/plus+ไทย"
```

```curl mdok name=query_9
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_9|string}}" --data "n=9"
```

```jmespath mdok check=query_9
status == `200`
body.query.n == '9'
```
