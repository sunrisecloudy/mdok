# T0158: header value case 8

<!-- mdok-corpus id=T0158 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_7
curl "{{base_url}}/echo" --header "X-Case: semi;colon"
```

```jmespath mdok check=header_7
status == `200`
length(body.headers."x-case") == `1`
```
