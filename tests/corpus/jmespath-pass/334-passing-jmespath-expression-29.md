# T0334: passing JMESPath expression 29

<!-- mdok-corpus id=T0334 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_28
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_28
length(headers."content-type") == `1`
```
