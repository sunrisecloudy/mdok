# T0005: valid executable fences with top-level heading 5

<!-- mdok-corpus id=T0005 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/4).

```bash
curl https://ignored.example/4
```

```curl mdok name=step_4
curl "{{base_url}}/echo?case=markdown-4"
```

```jmespath mdok check=step_4
status == `200`
```
