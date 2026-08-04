# T0347: failing JMESPath expression 12

<!-- mdok-corpus id=T0347 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_11
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_11
body.items[0].id == 'missing'
```
