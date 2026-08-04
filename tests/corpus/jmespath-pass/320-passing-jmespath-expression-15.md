# T0320: passing JMESPath expression 15

<!-- mdok-corpus id=T0320 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_14
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_14
contains(body.tags, 'blue')
```
