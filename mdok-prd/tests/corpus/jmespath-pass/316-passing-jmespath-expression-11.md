# T0316: passing JMESPath expression 11

<!-- mdok-corpus id=T0316 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_10
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_10
status == `200`
```
