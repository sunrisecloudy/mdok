# T0085: empty header value variant 25

<!-- mdok-corpus id=T0085 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_24
curl "{{base_url}}/echo" --header "X-Empty:"
```

```jmespath mdok check=shell_24
status == `200`
```
