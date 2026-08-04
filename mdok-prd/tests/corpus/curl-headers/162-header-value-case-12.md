# T0162: header value case 12

<!-- mdok-corpus id=T0162 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_11
curl "{{base_url}}/echo" --header "X-Case: with spaces"
```

```jmespath mdok check=header_11
status == `200`
length(body.headers."x-case") == `1`
```
