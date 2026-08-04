# T0341: failing JMESPath expression 6

<!-- mdok-corpus id=T0341 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_5
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_5
body.nested.value < `0`
```
