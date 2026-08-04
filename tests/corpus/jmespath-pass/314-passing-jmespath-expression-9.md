# T0314: passing JMESPath expression 9

<!-- mdok-corpus id=T0314 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_8
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_8
length(headers."content-type") == `1`
```
