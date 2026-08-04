# T0312: passing JMESPath expression 7

<!-- mdok-corpus id=T0312 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_6
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_6
type(body.object) == 'object'
```
