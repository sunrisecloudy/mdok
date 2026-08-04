# T0154: header value case 4

<!-- mdok-corpus id=T0154 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_3
curl "{{base_url}}/echo" --header "X-Case: quoted "value""
```

```jmespath mdok check=header_3
status == `200`
length(body.headers."x-case") == `1`
```
