# T0065: empty header value variant 5

<!-- mdok-corpus id=T0065 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_4
curl "{{base_url}}/echo" --header "X-Empty:"
```

```jmespath mdok check=shell_4
status == `200`
```
