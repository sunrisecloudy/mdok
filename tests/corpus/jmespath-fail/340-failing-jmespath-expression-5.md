# T0340: failing JMESPath expression 5

<!-- mdok-corpus id=T0340 category=jmespath-fail stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=json_fail_4
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_fail_4
contains(body.tags, 'not-present')
```
