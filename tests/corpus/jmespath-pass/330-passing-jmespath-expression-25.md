# T0330: passing JMESPath expression 25

<!-- mdok-corpus id=T0330 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_24
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_24
contains(body.tags, 'blue')
```
