# T0402: header template filter 17

<!-- mdok-corpus id=T0402 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_16 = "safe header value"
```

```curl mdok name=template_16
curl "{{base_url}}/echo?case=template-16" --header "X-Value: {{value_16|header}}"
```

```jmespath mdok check=template_16
status == `200`
```
