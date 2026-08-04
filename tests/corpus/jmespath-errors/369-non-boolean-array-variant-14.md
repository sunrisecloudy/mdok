# T0369: non boolean array variant 14

<!-- mdok-corpus id=T0369 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_13
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_13
body.items
```
