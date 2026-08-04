# T0203: query and url encoding 3

<!-- mdok-corpus id=T0203 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_2 = "space slash/plus+ไทย"
```

```curl mdok name=query_2
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_2|string}}" --data "n=2"
```

```jmespath mdok check=query_2
status == `200`
body.query.n == '2'
```
