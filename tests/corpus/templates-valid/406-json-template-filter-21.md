# T0406: json template filter 21

<!-- mdok-corpus id=T0406 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_20 = "quote \" and newline\\n"
```

```curl mdok name=template_20
curl "{{base_url}}/echo?case=template-20" --header "X-Value: {{value_20|json}}"
```

```jmespath mdok check=template_20
status == `200`
```
