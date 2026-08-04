# T0348: failing JMESPath expression 13

<!-- mdok-corpus id=T0348 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_12
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_12
contains(body.tags, 'not-present')
```
