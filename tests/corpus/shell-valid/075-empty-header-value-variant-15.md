# T0075: empty header value variant 15

<!-- mdok-corpus id=T0075 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_14
curl "{{base_url}}/echo" --header "X-Empty:"
```

```jmespath mdok check=shell_14
status == `200`
```
