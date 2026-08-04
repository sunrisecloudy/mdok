# T0307: passing JMESPath expression 2

<!-- mdok-corpus id=T0307 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_1
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_1
body.ok == `true`
```
