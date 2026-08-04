# T0156: header value case 6

<!-- mdok-corpus id=T0156 category=curl-headers stage=execute expected=pass -->

```curl mdok name=header_5
curl "{{base_url}}/echo" --header "X-Empty:"
```

```jmespath mdok check=header_5
status == `200`
length(body.headers."x-empty") == `1`
```
