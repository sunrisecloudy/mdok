# T0335: passing JMESPath expression 30

<!-- mdok-corpus id=T0335 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_29
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_29
timings.total_ms >= `0`
```
