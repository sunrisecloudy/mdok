# T0405: raw template filter 20

<!-- mdok-corpus id=T0405 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_19 = "semi;$(not-run)"
```

```curl mdok name=template_19
curl "{{base_url}}/echo?case=template-19" --header "X-Value: {{value_19|raw}}"
```

```jmespath mdok check=template_19
status == `200`
```
