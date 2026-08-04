# T0014: valid executable fences with nested heading 14

<!-- mdok-corpus id=T0014 category=markdown-valid stage=plan expected=pass -->

## Section

Ordinary prose with `inline code` and a [link](https://example.invalid/13).

```bash
curl https://ignored.example/13
```

```curl mdok name=step_13
curl "{{base_url}}/echo?case=markdown-13"
```

```jmespath mdok check=step_13
status == `200`
```
