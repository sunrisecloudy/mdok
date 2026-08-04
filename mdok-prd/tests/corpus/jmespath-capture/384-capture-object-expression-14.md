# T0384: capture object expression 14

<!-- mdok-corpus id=T0384 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_13
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_13
{flag: body.ok, count: length(body.items)}
```

```curl mdok name=use_13
curl "{{base_url}}/echo?case=capture-13"
```

```jmespath mdok check=use_13
status == `200`
```
