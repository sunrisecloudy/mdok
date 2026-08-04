# T0383: capture object expression 13

<!-- mdok-corpus id=T0383 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_12
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_12
{nested: body.nested}
```

```curl mdok name=use_12
curl "{{base_url}}/echo?case=capture-12"
```

```jmespath mdok check=use_12
status == `200`
```
