# T0399: raw template filter 14

<!-- mdok-corpus id=T0399 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_13 = "semi;$(not-run)"
```

```curl mdok name=template_13
curl "{{base_url}}/echo?case=template-13" --header "X-Value: {{value_13|raw}}"
```

```jmespath mdok check=template_13
status == `200`
```
