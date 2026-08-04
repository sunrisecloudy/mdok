# T0167: header value case 17

<!-- mdok-corpus id=T0167 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_16
curl "{{base_url}}/echo" --header "X-Case: colon:inside"
```

```jmespath mdok check=header_16
status == `200`
length(body.headers."x-case") == `1`
```
