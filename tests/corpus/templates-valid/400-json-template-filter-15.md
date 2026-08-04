# T0400: json template filter 15

<!-- mdok-corpus id=T0400 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_14 = "quote \" and newline\\n"
```

```curl mdok name=template_14
curl "{{base_url}}/echo?case=template-14" --header "X-Value: {{value_14|json}}"
```

```jmespath mdok check=template_14
status == `200`
```
