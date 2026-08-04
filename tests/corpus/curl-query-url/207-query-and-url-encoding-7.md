# T0207: query and url encoding 7

<!-- mdok-corpus id=T0207 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_6 = "space slash/plus+ไทย"
```

```curl mdok name=query_6
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_6|string}}" --data "n=6"
```

```jmespath mdok check=query_6
status == `200`
body.query.n == '6'
```
