# T0351: failing JMESPath expression 16

<!-- mdok-corpus id=T0351 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_15
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_15
timings.total_ms < `0`
```
