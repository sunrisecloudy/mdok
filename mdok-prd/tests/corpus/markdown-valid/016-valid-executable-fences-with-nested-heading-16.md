# T0016: valid executable fences with nested heading 16

<!-- mdok-corpus id=T0016 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/15).

```bash
curl https://ignored.example/15
```

```curl mdok name=step_15
curl "{{base_url}}/echo?case=markdown-15"
```

```jmespath mdok check=step_15
status == `200`
```
