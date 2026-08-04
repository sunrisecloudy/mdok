# T0359: non boolean string variant 4

<!-- mdok-corpus id=T0359 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_3
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_3
body.name
```
