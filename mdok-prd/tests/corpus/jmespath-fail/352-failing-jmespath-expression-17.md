# T0352: failing JMESPath expression 17

<!-- mdok-corpus id=T0352 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_16
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_16
status == `201`
```
