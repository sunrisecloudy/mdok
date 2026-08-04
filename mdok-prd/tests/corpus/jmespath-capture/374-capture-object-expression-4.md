# T0374: capture object expression 4

<!-- mdok-corpus id=T0374 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_3
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_3
{flag: body.ok, count: length(body.items)}
```

```curl mdok name=use_3
curl "{{base_url}}/echo?case=capture-3"
```

```jmespath mdok check=use_3
status == `200`
```
