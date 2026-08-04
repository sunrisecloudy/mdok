# T0356: invalid bracket variant 1

<!-- mdok-corpus id=T0356 category=jmespath-errors stage=plan expected=error error=MDOK-E500 -->

```curl mdok name=jerr_0
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_0
body.items[
```
