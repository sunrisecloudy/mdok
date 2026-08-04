# T0003: valid executable fences with top-level heading 3

<!-- mdok-corpus id=T0003 category=markdown-valid stage=plan expected=pass -->

# API

Ordinary prose with `inline code` and a [link](https://example.invalid/2).

```bash
curl https://ignored.example/2
```

```curl mdok name=step_2
curl "{{base_url}}/echo?case=markdown-2"
```

```jmespath mdok check=step_2
status == `200`
```
