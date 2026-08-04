# T0404: string template filter 19

<!-- mdok-corpus id=T0404 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_18 = "hello world"
```

```curl mdok name=template_18
curl "{{base_url}}/echo?case=template-18" --header "X-Value: {{value_18|string}}"
```

```jmespath mdok check=template_18
status == `200`
```
