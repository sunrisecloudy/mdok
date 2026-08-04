# T0368: non boolean object variant 13

<!-- mdok-corpus id=T0368 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_12
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_12
body
```
