# T0390: header template filter 5

<!-- mdok-corpus id=T0390 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_4 = "safe header value"
```

```curl mdok name=template_4
curl "{{base_url}}/echo?case=template-4" --header "X-Value: {{value_4|header}}"
```

```jmespath mdok check=template_4
status == `200`
```
