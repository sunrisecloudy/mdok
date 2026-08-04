# T0008: valid executable fences with nested heading 8

<!-- mdok-corpus id=T0008 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/7).

```bash
curl https://ignored.example/7
```

```curl mdok name=step_7
curl "{{base_url}}/echo?case=markdown-7"
```

```jmespath mdok check=step_7
status == `200`
```
