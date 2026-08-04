# T0315: passing JMESPath expression 10

<!-- mdok-corpus id=T0315 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_9
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_9
timings.total_ms >= `0`
```
