# T0012: valid executable fences with nested heading 12

<!-- mdok-corpus id=T0012 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/11).

```bash
curl https://ignored.example/11
```

```curl mdok name=step_11
curl "{{base_url}}/echo?case=markdown-11"
```

```jmespath mdok check=step_11
status == `200`
```
