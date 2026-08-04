# T0324: passing JMESPath expression 19

<!-- mdok-corpus id=T0324 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_18
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_18
length(headers."content-type") == `1`
```
