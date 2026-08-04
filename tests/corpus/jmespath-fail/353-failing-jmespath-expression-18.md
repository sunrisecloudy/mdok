# T0353: failing JMESPath expression 18

<!-- mdok-corpus id=T0353 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_17
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_17
body.ok == `false`
```
