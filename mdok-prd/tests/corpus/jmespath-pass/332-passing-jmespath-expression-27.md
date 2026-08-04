# T0332: passing JMESPath expression 27

<!-- mdok-corpus id=T0332 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_26
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_26
type(body.object) == 'object'
```
