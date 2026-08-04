# T0357: invalid operator variant 2

<!-- mdok-corpus id=T0357 category=jmespath-errors stage=plan expected=error error=MDOK-E500 -->

```curl mdok name=jerr_1
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_1
status === `200`
```
