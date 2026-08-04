# T0019: valid executable fences with top-level heading 19

<!-- mdok-corpus id=T0019 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/18).

```bash
curl https://ignored.example/18
```

```curl mdok name=step_18
curl "{{base_url}}/echo?case=markdown-18"
```

```jmespath mdok check=step_18
status == `200`
```
