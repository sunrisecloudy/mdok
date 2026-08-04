# T0151: header value case 1

<!-- mdok-corpus id=T0151 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_0
curl "{{base_url}}/echo" --header "X-Case: simple"
```

```jmespath mdok check=header_0
status == `200`
length(body.headers."x-case") == `1`
```
