# T0394: json template filter 9

<!-- mdok-corpus id=T0394 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_8 = "quote \" and newline\\n"
```

```curl mdok name=template_8
curl "{{base_url}}/echo?case=template-8" --header "X-Value: {{value_8|json}}"
```

```jmespath mdok check=template_8
status == `200`
```
