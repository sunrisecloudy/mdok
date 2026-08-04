# T0372: capture object expression 2

<!-- mdok-corpus id=T0372 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_1
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_1
{ids: body.items[].id}
```

```curl mdok name=use_1
curl "{{base_url}}/echo?case=capture-1"
```

```jmespath mdok check=use_1
status == `200`
```
