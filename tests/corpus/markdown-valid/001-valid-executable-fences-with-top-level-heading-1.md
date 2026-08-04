# T0001: valid executable fences with top-level heading 1

<!-- mdok-corpus id=T0001 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/0).

```bash
curl https://ignored.example/0
```

```curl mdok name=step_0
curl "{{base_url}}/echo?case=markdown-0"
```

```jmespath mdok check=step_0
status == `200`
```
