# T0387: raw template filter 2

<!-- mdok-corpus id=T0387 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_1 = "semi;$(not-run)"
```

```curl mdok name=template_1
curl "{{base_url}}/echo?case=template-1" --header "X-Value: {{value_1|raw}}"
```

```jmespath mdok check=template_1
status == `200`
```
