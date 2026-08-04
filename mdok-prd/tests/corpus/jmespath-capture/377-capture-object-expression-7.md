# T0377: capture object expression 7

<!-- mdok-corpus id=T0377 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_6
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_6
{ids: body.items[].id}
```

```curl mdok name=use_6
curl "{{base_url}}/echo?case=capture-6"
```

```jmespath mdok check=use_6
status == `200`
```
