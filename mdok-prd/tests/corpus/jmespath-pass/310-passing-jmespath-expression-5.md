# T0310: passing JMESPath expression 5

<!-- mdok-corpus id=T0310 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_4
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_4
contains(body.tags, 'blue')
```
