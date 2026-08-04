# T0306: passing JMESPath expression 1

<!-- mdok-corpus id=T0306 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_0
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_0
status == `200`
```
