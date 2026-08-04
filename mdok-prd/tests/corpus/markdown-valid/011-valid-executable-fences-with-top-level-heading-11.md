# T0011: valid executable fences with top-level heading 11

<!-- mdok-corpus id=T0011 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/10).

```bash
curl https://ignored.example/10
```

```curl mdok name=step_10
curl "{{base_url}}/echo?case=markdown-10"
```

```jmespath mdok check=step_10
status == `200`
```
