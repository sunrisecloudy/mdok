# T0205: query and url encoding 5

<!-- mdok-corpus id=T0205 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_4 = "space slash/plus+ไทย"
```

```curl mdok name=query_4
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_4|string}}" --data "n=4"
```

```jmespath mdok check=query_4
status == `200`
body.query.n == '4'
```
