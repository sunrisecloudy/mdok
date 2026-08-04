# T0376: capture object expression 6

<!-- mdok-corpus id=T0376 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_5
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_5
{id: body.items[0].id}
```

```curl mdok name=use_5
curl "{{base_url}}/echo?case=capture-5"
```

```jmespath mdok check=use_5
status == `200`
```
