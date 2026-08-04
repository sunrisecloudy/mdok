# T0386: string template filter 1

<!-- mdok-corpus id=T0386 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_0 = "hello world"
```

```curl mdok name=template_0
curl "{{base_url}}/echo?case=template-0" --header "X-Value: {{value_0|string}}"
```

```jmespath mdok check=template_0
status == `200`
```
