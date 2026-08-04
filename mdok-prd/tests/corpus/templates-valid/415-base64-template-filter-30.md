# T0415: base64 template filter 30

<!-- mdok-corpus id=T0415 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_29 = "binary-ish\\u0000text"
```

```curl mdok name=template_29
curl "{{base_url}}/echo?case=template-29" --header "X-Value: {{value_29|base64}}"
```

```jmespath mdok check=template_29
status == `200`
```
