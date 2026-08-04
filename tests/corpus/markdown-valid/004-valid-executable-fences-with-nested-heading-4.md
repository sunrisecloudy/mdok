# T0004: valid executable fences with nested heading 4

<!-- mdok-corpus id=T0004 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/3).

```bash
curl https://ignored.example/3
```

```curl mdok name=step_3
curl "{{base_url}}/echo?case=markdown-3"
```

```jmespath mdok check=step_3
status == `200`
```
