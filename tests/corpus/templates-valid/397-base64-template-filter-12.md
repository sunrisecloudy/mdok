# T0397: base64 template filter 12

<!-- mdok-corpus id=T0397 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_11 = "binary-ish\\u0000text"
```

```curl mdok name=template_11
curl "{{base_url}}/echo?case=template-11" --header "X-Value: {{value_11|base64}}"
```

```jmespath mdok check=template_11
status == `200`
```
