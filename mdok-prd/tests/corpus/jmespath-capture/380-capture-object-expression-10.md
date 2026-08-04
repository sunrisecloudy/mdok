# T0380: capture object expression 10

<!-- mdok-corpus id=T0380 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_9
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_9
{first_blue: body.items[?color == `blue`] | [0].id}
```

```curl mdok name=use_9
curl "{{base_url}}/echo?case=capture-9"
```

```jmespath mdok check=use_9
status == `200`
```
