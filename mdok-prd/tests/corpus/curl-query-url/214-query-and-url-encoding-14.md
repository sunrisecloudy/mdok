# T0214: query and url encoding 14

<!-- mdok-corpus id=T0214 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_13 = "space slash/plus+ไทย"
```

```curl mdok name=query_13
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_13|string}}" --data "n=13"
```

```jmespath mdok check=query_13
status == `200`
body.query.n == '13'
```
