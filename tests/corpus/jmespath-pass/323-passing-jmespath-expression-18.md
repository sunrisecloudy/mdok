# T0323: passing JMESPath expression 18

<!-- mdok-corpus id=T0323 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_17
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_17
type(body.null_value) == 'null'
```
