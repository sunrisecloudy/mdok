# T0381: capture object expression 11

<!-- mdok-corpus id=T0381 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_10
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_10
{id: body.items[0].id}
```

```curl mdok name=use_10
curl "{{base_url}}/echo?case=capture-10"
```

```jmespath mdok check=use_10
status == `200`
```
