# T0168: header value case 18

<!-- mdok-corpus id=T0168 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_17
curl "{{base_url}}/echo" --header "X-Case: semi;colon"
```

```jmespath mdok check=header_17
status == `200`
length(body.headers."x-case") == `1`
```
