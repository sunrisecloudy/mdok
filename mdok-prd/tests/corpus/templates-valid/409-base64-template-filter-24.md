# T0409: base64 template filter 24

<!-- mdok-corpus id=T0409 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_23 = "binary-ish\\u0000text"
```

```curl mdok name=template_23
curl "{{base_url}}/echo?case=template-23" --header "X-Value: {{value_23|base64}}"
```

```jmespath mdok check=template_23
status == `200`
```
