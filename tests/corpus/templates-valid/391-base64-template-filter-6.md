# T0391: base64 template filter 6

<!-- mdok-corpus id=T0391 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_5 = "binary-ish\\u0000text"
```

```curl mdok name=template_5
curl "{{base_url}}/echo?case=template-5" --header "X-Value: {{value_5|base64}}"
```

```jmespath mdok check=template_5
status == `200`
```
