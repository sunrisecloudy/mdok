# T0160: header value case 10

<!-- mdok-corpus id=T0160 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_9
curl "{{base_url}}/echo" --header "X-Case: Bearer token"
```

```jmespath mdok check=header_9
status == `200`
length(body.headers."x-case") == `1`
```
