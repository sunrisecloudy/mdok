# T0410: string template filter 25

<!-- mdok-corpus id=T0410 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_24 = "hello world"
```

```curl mdok name=template_24
curl "{{base_url}}/echo?case=template-24" --header "X-Value: {{value_24|string}}"
```

```jmespath mdok check=template_24
status == `200`
```
