# T0321: passing JMESPath expression 16

<!-- mdok-corpus id=T0321 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_15
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_15
body.nested.value > `10`
```
