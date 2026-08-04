# T0412: json template filter 27

<!-- mdok-corpus id=T0412 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_26 = "quote \" and newline\\n"
```

```curl mdok name=template_26
curl "{{base_url}}/echo?case=template-26" --header "X-Value: {{value_26|json}}"
```

```jmespath mdok check=template_26
status == `200`
```
