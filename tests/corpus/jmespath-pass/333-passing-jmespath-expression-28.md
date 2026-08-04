# T0333: passing JMESPath expression 28

<!-- mdok-corpus id=T0333 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_27
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_27
type(body.null_value) == 'null'
```
