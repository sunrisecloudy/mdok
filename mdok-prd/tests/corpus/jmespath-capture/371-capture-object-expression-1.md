# T0371: capture object expression 1

<!-- mdok-corpus id=T0371 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_0
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_0
{id: body.items[0].id}
```

```curl mdok name=use_0
curl "{{base_url}}/echo?case=capture-0"
```

```jmespath mdok check=use_0
status == `200`
```
