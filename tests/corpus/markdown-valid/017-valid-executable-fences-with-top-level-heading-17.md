# T0017: valid executable fences with top-level heading 17

<!-- mdok-corpus id=T0017 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/16).

```bash
curl https://ignored.example/16
```

```curl mdok name=step_16
curl "{{base_url}}/echo?case=markdown-16"
```

```jmespath mdok check=step_16
status == `200`
```
