# T0342: failing JMESPath expression 7

<!-- mdok-corpus id=T0342 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_6
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_6
headers."x-missing" != null
```
