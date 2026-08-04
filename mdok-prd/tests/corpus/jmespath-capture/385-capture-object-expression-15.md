# T0385: capture object expression 15

<!-- mdok-corpus id=T0385 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_14
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_14
{first_blue: body.items[?color == `blue`] | [0].id}
```

```curl mdok name=use_14
curl "{{base_url}}/echo?case=capture-14"
```

```jmespath mdok check=use_14
status == `200`
```
