# T0375: capture object expression 5

<!-- mdok-corpus id=T0375 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_4
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_4
{first_blue: body.items[?color == `blue`] | [0].id}
```

```curl mdok name=use_4
curl "{{base_url}}/echo?case=capture-4"
```

```jmespath mdok check=use_4
status == `200`
```
