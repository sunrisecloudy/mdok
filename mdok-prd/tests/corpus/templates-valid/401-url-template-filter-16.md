# T0401: url template filter 16

<!-- mdok-corpus id=T0401 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_15 = "space/slash+ไทย"
```

```curl mdok name=template_15
curl "{{base_url}}/echo?case=template-15" --header "X-Value: {{value_15|url}}"
```

```jmespath mdok check=template_15
status == `200`
```
