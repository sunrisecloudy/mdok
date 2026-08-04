# T0345: failing JMESPath expression 10

<!-- mdok-corpus id=T0345 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_9
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_9
body.ok == `false`
```
