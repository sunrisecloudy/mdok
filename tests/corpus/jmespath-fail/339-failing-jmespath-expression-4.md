# T0339: failing JMESPath expression 4

<!-- mdok-corpus id=T0339 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_3
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_3
body.items[0].id == 'missing'
```
