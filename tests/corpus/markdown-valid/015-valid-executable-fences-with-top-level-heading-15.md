# T0015: valid executable fences with top-level heading 15

<!-- mdok-corpus id=T0015 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/14).

```bash
curl https://ignored.example/14
```

```curl mdok name=step_14
curl "{{base_url}}/echo?case=markdown-14"
```

```jmespath mdok check=step_14
status == `200`
```
