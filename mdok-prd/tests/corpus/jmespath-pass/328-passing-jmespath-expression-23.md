# T0328: passing JMESPath expression 23

<!-- mdok-corpus id=T0328 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_22
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_22
length(body.items) == `3`
```
