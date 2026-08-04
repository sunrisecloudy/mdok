# T0209: query and url encoding 9

<!-- mdok-corpus id=T0209 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_8 = "space slash/plus+ไทย"
```

```curl mdok name=query_8
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_8|string}}" --data "n=8"
```

```jmespath mdok check=query_8
status == `200`
body.query.n == '8'
```
