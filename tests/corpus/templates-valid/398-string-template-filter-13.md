# T0398: string template filter 13

<!-- mdok-corpus id=T0398 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_12 = "hello world"
```

```curl mdok name=template_12
curl "{{base_url}}/echo?case=template-12" --header "X-Value: {{value_12|string}}"
```

```jmespath mdok check=template_12
status == `200`
```
