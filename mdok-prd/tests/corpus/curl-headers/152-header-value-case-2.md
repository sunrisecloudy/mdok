# T0152: header value case 2

<!-- mdok-corpus id=T0152 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_1
curl "{{base_url}}/echo" --header "X-Case: with spaces"
```

```jmespath mdok check=header_1
status == `200`
length(body.headers."x-case") == `1`
```
