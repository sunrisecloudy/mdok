# T0010: valid executable fences with nested heading 10

<!-- mdok-corpus id=T0010 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/9).

```bash
curl https://ignored.example/9
```

```curl mdok name=step_9
curl "{{base_url}}/echo?case=markdown-9"
```

```jmespath mdok check=step_9
status == `200`
```
