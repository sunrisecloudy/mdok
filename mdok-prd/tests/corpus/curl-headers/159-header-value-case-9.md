# T0159: header value case 9

<!-- mdok-corpus id=T0159 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_8
curl "{{base_url}}/echo" --header "X-Case: tab\tvalue"
```

```jmespath mdok check=header_8
status == `200`
length(body.headers."x-case") == `1`
```
