# T0392: string template filter 7

<!-- mdok-corpus id=T0392 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_6 = "hello world"
```

```curl mdok name=template_6
curl "{{base_url}}/echo?case=template-6" --header "X-Value: {{value_6|string}}"
```

```jmespath mdok check=template_6
status == `200`
```
