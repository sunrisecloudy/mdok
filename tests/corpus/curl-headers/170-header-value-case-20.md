# T0170: header value case 20

<!-- mdok-corpus id=T0170 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_19
curl "{{base_url}}/echo" --header "X-Case: Bearer token"
```

```jmespath mdok check=header_19
status == `200`
length(body.headers."x-case") == `1`
```
