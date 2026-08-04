# T0364: invalid bracket variant 9

<!-- mdok-corpus id=T0364 category=jmespath-errors stage=plan expected=error error=MDOK-E500 -->

```curl mdok name=jerr_8
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_8
body.items[
```
