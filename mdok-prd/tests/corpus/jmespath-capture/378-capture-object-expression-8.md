# T0378: capture object expression 8

<!-- mdok-corpus id=T0378 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_7
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_7
{nested: body.nested}
```

```curl mdok name=use_7
curl "{{base_url}}/echo?case=capture-7"
```

```jmespath mdok check=use_7
status == `200`
```
