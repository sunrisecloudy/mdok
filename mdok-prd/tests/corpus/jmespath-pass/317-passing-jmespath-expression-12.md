# T0317: passing JMESPath expression 12

<!-- mdok-corpus id=T0317 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_11
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_11
body.ok == `true`
```
