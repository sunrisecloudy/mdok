# T0367: non boolean string variant 12

<!-- mdok-corpus id=T0367 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_11
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_11
body.name
```
