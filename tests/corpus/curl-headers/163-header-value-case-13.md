# T0163: header value case 13

<!-- mdok-corpus id=T0163 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_12
curl "{{base_url}}/echo" --header "X-Case: comma,value"
```

```jmespath mdok check=header_12
status == `200`
length(body.headers."x-case") == `1`
```
