# T0393: raw template filter 8

<!-- mdok-corpus id=T0393 category=templates-valid stage=execute expected=pass -->

```toml mdok vars
value_7 = "semi;$(not-run)"
```

```curl mdok name=template_7
curl "{{base_url}}/echo?case=template-7" --header "X-Value: {{value_7|raw}}"
```

```jmespath mdok check=template_7
status == `200`
```
