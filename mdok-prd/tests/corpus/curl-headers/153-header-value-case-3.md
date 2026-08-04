# T0153: header value case 3

<!-- mdok-corpus id=T0153 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_2
curl "{{base_url}}/echo" --header "X-Case: comma,value"
```

```jmespath mdok check=header_2
status == `200`
length(body.headers."x-case") == `1`
```
