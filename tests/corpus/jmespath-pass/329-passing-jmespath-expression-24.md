# T0329: passing JMESPath expression 24

<!-- mdok-corpus id=T0329 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_23
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_23
body.items[0].id == 'a'
```
