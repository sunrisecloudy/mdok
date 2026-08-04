# T0002: valid executable fences with nested heading 2

<!-- mdok-corpus id=T0002 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/1).

```bash
curl https://ignored.example/1
```

```curl mdok name=step_1
curl "{{base_url}}/echo?case=markdown-1"
```

```jmespath mdok check=step_1
status == `200`
```
