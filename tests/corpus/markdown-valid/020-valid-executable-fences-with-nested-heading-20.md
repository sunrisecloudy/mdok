# T0020: valid executable fences with nested heading 20

<!-- mdok-corpus id=T0020 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/19).

```bash
curl https://ignored.example/19
```

```curl mdok name=step_19
curl "{{base_url}}/echo?case=markdown-19"
```

```jmespath mdok check=step_19
status == `200`
```
