# T0355: failing JMESPath expression 20

<!-- mdok-corpus id=T0355 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_19
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_19
body.items[0].id == 'missing'
```
