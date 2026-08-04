# T0411: raw template filter 26

<!-- mdok-corpus id=T0411 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_25 = "semi;$(not-run)"
```

```curl mdok name=template_25
curl "{{base_url}}/echo?case=template-25" --header "X-Value: {{value_25|raw}}"
```

```jmespath mdok check=template_25
status == `200`
```
