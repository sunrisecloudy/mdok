# T0344: failing JMESPath expression 9

<!-- mdok-corpus id=T0344 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_8
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_8
status == `201`
```
