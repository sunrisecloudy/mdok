# T0157: header value case 7

<!-- mdok-corpus id=T0157 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_6
curl "{{base_url}}/echo" --header "X-Case: colon:inside"
```

```jmespath mdok check=header_6
status == `200`
length(body.headers."x-case") == `1`
```
