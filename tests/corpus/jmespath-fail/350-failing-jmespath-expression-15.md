# T0350: failing JMESPath expression 15

<!-- mdok-corpus id=T0350 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_14
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_14
headers."x-missing" != null
```
