# T0413: url template filter 28

<!-- mdok-corpus id=T0413 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_27 = "space/slash+ไทย"
```

```curl mdok name=template_27
curl "{{base_url}}/echo?case=template-27" --header "X-Value: {{value_27|url}}"
```

```jmespath mdok check=template_27
status == `200`
```
