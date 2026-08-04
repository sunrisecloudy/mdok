# T0358: unterminated literal variant 3

<!-- mdok-corpus id=T0358 category=jmespath-errors stage=plan expected=error error=MDOK-E500 -->

```curl mdok name=jerr_2
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_2
body.name == 'x
```
