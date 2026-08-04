# T0407: url template filter 22

<!-- mdok-corpus id=T0407 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_21 = "space/slash+ไทย"
```

```curl mdok name=template_21
curl "{{base_url}}/echo?case=template-21" --header "X-Value: {{value_21|url}}"
```

```jmespath mdok check=template_21
status == `200`
```
