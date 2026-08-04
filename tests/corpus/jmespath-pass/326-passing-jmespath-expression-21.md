# T0326: passing JMESPath expression 21

<!-- mdok-corpus id=T0326 category=jmespath-pass stage=execute expected=pass -->

```curl mdok name=json_20
curl "{{base_url}}/json/standard"
```

```jmespath mdok check=json_20
status == `200`
```
