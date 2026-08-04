# T0343: failing JMESPath expression 8

<!-- mdok-corpus id=T0343 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_7
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_7
timings.total_ms < `0`
```
