# T0007: valid executable fences with top-level heading 7

<!-- mdok-corpus id=T0007 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/6).

```bash
curl https://ignored.example/6
```

```curl mdok name=step_6
curl "{{base_url}}/echo?case=markdown-6"
```

```jmespath mdok check=step_6
status == `200`
```
