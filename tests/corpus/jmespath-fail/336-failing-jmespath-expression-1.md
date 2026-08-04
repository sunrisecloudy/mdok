# T0336: failing JMESPath expression 1

<!-- mdok-corpus id=T0336 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_0
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_0
status == `201`
```
