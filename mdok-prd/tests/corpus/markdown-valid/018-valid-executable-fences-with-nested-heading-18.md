# T0018: valid executable fences with nested heading 18

<!-- mdok-corpus id=T0018 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/17).

```bash
curl https://ignored.example/17
```

```curl mdok name=step_17
curl "{{base_url}}/echo?case=markdown-17"
```

```jmespath mdok check=step_17
status == `200`
```
