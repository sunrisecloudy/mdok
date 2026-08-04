# T0365: invalid operator variant 10

<!-- mdok-corpus id=T0365 category=jmespath-errors stage=plan expected=error error=MDOK-E500 -->

```curl mdok name=jerr_9
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_9
status === `200`
```
