# T0155: header value case 5

<!-- mdok-corpus id=T0155 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_4
curl "{{base_url}}/echo" --header "X-Case: unicode ไทย"
```

```jmespath mdok check=header_4
status == `200`
length(body.headers."x-case") == `1`
```
