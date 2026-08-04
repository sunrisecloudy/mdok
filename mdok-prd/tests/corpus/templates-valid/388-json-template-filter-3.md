# T0388: json template filter 3

<!-- mdok-corpus id=T0388 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_2 = "quote \" and newline\\n"
```

```curl mdok name=template_2
curl "{{base_url}}/echo?case=template-2" --header "X-Value: {{value_2|json}}"
```

```jmespath mdok check=template_2
status == `200`
```
