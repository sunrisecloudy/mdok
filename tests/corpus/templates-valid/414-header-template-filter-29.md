# T0414: header template filter 29

<!-- mdok-corpus id=T0414 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_28 = "safe header value"
```

```curl mdok name=template_28
curl "{{base_url}}/echo?case=template-28" --header "X-Value: {{value_28|header}}"
```

```jmespath mdok check=template_28
status == `200`
```
