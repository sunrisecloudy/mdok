# T0319: passing JMESPath expression 14

<!-- mdok-corpus id=T0319 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_13
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_13
body.items[0].id == 'a'
```
