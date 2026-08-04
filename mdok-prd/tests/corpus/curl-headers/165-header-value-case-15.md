# T0165: header value case 15

<!-- mdok-corpus id=T0165 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_14
curl "{{base_url}}/echo" --header "X-Case: unicode ไทย"
```

```jmespath mdok check=header_14
status == `200`
length(body.headers."x-case") == `1`
```
