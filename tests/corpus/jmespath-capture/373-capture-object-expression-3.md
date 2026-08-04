# T0373: capture object expression 3

<!-- mdok-corpus id=T0373 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_2
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_2
{nested: body.nested}
```

```curl mdok name=use_2
curl "{{base_url}}/echo?case=capture-2"
```

```jmespath mdok check=use_2
status == `200`
```
