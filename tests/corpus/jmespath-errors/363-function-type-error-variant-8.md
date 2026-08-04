# T0363: function type error variant 8

<!-- mdok-corpus id=T0363 category=jmespath-errors stage=execute expected=error error=MDOK-E501 -->

```curl mdok name=jerr_7
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=jerr_7
length(status)
```
