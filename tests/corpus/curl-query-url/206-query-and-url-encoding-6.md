# T0206: query and url encoding 6

<!-- mdok-corpus id=T0206 category=curl-query-url stage=execute expected=pass -->

```toml mdok vars
query_value_5 = "space slash/plus+ไทย"
```

```curl mdok name=query_5
curl --get "{{base_url}}/echo" --data-urlencode "q={{query_value_5|string}}" --data "n=5"
```

```jmespath mdok check=query_5
status == `200`
body.query.n == '5'
```
