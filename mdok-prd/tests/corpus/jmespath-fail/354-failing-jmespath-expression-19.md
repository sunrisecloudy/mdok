# T0354: failing JMESPath expression 19

<!-- mdok-corpus id=T0354 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_18
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_18
length(body.items) == `99`
```
