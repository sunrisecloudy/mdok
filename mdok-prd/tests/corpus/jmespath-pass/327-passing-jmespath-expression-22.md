# T0327: passing JMESPath expression 22

<!-- mdok-corpus id=T0327 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_21
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_21
body.ok == `true`
```
