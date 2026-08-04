# T0169: header value case 19

<!-- mdok-corpus id=T0169 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_18
curl "{{base_url}}/echo" --header "X-Case: tab\tvalue"
```

```jmespath mdok check=header_18
status == `200`
length(body.headers."x-case") == `1`
```
