# T0013: valid executable fences with top-level heading 13

<!-- mdok-corpus id=T0013 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/12).

```bash
curl https://ignored.example/12
```

```curl mdok name=step_12
curl "{{base_url}}/echo?case=markdown-12"
```

```jmespath mdok check=step_12
status == `200`
```
