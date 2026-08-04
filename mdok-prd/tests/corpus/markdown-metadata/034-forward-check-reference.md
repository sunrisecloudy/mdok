# T0034: forward check reference

<!-- mdok-corpus id=T0034 category=markdown-metadata stage=plan expected=error error=MDOK-E102 -->

```jmespath mdok check=later
status == `200`
```

```curl mdok name=later
curl "{{base_url}}/echo"
```
