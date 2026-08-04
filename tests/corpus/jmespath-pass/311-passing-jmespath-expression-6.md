# T0311: passing JMESPath expression 6

<!-- mdok-corpus id=T0311 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_5
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_5
body.nested.value > `10`
```
