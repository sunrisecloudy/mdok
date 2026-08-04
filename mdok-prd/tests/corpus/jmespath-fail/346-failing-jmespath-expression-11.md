# T0346: failing JMESPath expression 11

<!-- mdok-corpus id=T0346 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_10
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_10
length(body.items) == `99`
```
