# T0349: failing JMESPath expression 14

<!-- mdok-corpus id=T0349 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_13
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_13
body.nested.value < `0`
```
