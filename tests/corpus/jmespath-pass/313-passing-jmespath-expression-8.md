# T0313: passing JMESPath expression 8

<!-- mdok-corpus id=T0313 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_7
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_7
type(body.null_value) == 'null'
```
