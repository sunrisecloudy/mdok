# T0389: url template filter 4

<!-- mdok-corpus id=T0389 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_3 = "space/slash+ไทย"
```

```curl mdok name=template_3
curl "{{base_url}}/echo?case=template-3" --header "X-Value: {{value_3|url}}"
```

```jmespath mdok check=template_3
status == `200`
```
