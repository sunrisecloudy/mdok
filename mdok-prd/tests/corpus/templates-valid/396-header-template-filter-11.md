# T0396: header template filter 11

<!-- mdok-corpus id=T0396 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_10 = "safe header value"
```

```curl mdok name=template_10
curl "{{base_url}}/echo?case=template-10" --header "X-Value: {{value_10|header}}"
```

```jmespath mdok check=template_10
status == `200`
```
