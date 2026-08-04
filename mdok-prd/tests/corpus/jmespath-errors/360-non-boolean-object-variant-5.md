# T0360: non boolean object variant 5

<!-- mdok-corpus id=T0360 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_4
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_4
body
```
