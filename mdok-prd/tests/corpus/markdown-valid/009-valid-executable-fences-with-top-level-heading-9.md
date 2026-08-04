# T0009: valid executable fences with top-level heading 9

<!-- mdok-corpus id=T0009 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/8).

```bash
curl https://ignored.example/8
```

```curl mdok name=step_8
curl "{{base_url}}/echo?case=markdown-8"
```

```jmespath mdok check=step_8
status == `200`
```
