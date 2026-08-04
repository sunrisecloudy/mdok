# T0403: base64 template filter 18

<!-- mdok-corpus id=T0403 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_17 = "binary-ish\\u0000text"
```

```curl mdok name=template_17
curl "{{base_url}}/echo?case=template-17" --header "X-Value: {{value_17|base64}}"
```

```jmespath mdok check=template_17
status == `200`
```
