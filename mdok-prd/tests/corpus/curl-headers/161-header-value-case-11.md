# T0161: header value case 11

<!-- mdok-corpus id=T0161 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_10
curl "{{base_url}}/echo" --header "X-Case: simple"
```

```jmespath mdok check=header_10
status == `200`
length(body.headers."x-case") == `1`
```
