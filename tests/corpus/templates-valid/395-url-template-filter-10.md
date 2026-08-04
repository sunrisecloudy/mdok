# T0395: url template filter 10

<!-- mdok-corpus id=T0395 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_9 = "space/slash+ไทย"
```

```curl mdok name=template_9
curl "{{base_url}}/echo?case=template-9" --header "X-Value: {{value_9|url}}"
```

```jmespath mdok check=template_9
status == `200`
```
