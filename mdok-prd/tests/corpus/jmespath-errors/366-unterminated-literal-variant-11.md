# T0366: unterminated literal variant 11

<!-- mdok-corpus id=T0366 category=jmespath-errors stage=plan expected=error error=MDOK-E500 -->

```curl mdok name=jerr_10
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_10
body.name == 'x
```
