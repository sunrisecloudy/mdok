# T0337: failing JMESPath expression 2

<!-- mdok-corpus id=T0337 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_1
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_1
body.ok == `false`
```
