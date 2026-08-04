# T0006: valid executable fences with nested heading 6

<!-- mdok-corpus id=T0006 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/5).

```bash
curl https://ignored.example/5
```

```curl mdok name=step_5
curl "{{base_url}}/echo?case=markdown-5"
```

```jmespath mdok check=step_5
status == `200`
```
