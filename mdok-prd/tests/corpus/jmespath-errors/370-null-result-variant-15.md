# T0370: null result variant 15

<!-- mdok-corpus id=T0370 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_14
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_14
body.missing
```
