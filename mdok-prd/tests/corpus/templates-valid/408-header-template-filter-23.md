# T0408: header template filter 23

<!-- mdok-corpus id=T0408 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_22 = "safe header value"
```

```curl mdok name=template_22
curl "{{base_url}}/echo?case=template-22" --header "X-Value: {{value_22|header}}"
```

```jmespath mdok check=template_22
status == `200`
```
