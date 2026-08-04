# T0331: passing JMESPath expression 26

<!-- mdok-corpus id=T0331 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_25
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_25
body.nested.value > `10`
```
