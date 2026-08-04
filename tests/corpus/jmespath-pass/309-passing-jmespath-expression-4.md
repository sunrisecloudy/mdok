# T0309: passing JMESPath expression 4

<!-- mdok-corpus id=T0309 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_3
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_3
body.items[0].id == 'a'
```
