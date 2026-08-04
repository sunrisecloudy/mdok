# T0338: failing JMESPath expression 3

<!-- mdok-corpus id=T0338 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_2
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_2
length(body.items) == `99`
```
