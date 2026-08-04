# T0308: passing JMESPath expression 3

<!-- mdok-corpus id=T0308 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_2
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_2
length(body.items) == `3`
```
