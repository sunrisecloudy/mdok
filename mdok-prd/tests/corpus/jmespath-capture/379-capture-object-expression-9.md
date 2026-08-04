# T0379: capture object expression 9

<!-- mdok-corpus id=T0379 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_8
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_8
{flag: body.ok, count: length(body.items)}
```

```curl mdok name=use_8
curl "{{base_url}}/echo?case=capture-8"
```

```jmespath mdok check=use_8
status == `200`
```
