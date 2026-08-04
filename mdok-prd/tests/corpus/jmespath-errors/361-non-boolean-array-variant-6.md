# T0361: non boolean array variant 6

<!-- mdok-corpus id=T0361 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_5
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_5
body.items
```
