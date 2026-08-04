# T0166: header value case 16

<!-- mdok-corpus id=T0166 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_15
curl "{{base_url}}/echo" --header "X-Empty:"
```

```jmespath mdok check=header_15
status == `200`
length(body.headers."x-empty") == `1`
```
