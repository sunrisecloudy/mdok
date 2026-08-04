# T0362: null result variant 7

<!-- mdok-corpus id=T0362 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_6
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_6
body.missing
```
