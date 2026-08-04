# T0325: passing JMESPath expression 20

<!-- mdok-corpus id=T0325 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_19
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_19
timings.total_ms >= `0`
```
