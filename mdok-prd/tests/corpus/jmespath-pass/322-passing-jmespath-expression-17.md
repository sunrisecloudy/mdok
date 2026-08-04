# T0322: passing JMESPath expression 17

<!-- mdok-corpus id=T0322 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_16
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_16
type(body.object) == 'object'
```
