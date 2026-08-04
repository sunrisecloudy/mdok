# T0164: header value case 14

<!-- mdok-corpus id=T0164 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_13
curl "{{base_url}}/echo" --header "X-Case: quoted "value""
```

```jmespath mdok check=header_13
status == `200`
length(body.headers."x-case") == `1`
```
