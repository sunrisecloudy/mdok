# T0318: passing JMESPath expression 13

<!-- mdok-corpus id=T0318 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_12
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_12
length(body.items) == `3`
```
