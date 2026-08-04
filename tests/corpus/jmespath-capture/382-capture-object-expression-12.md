# T0382: capture object expression 12

<!-- mdok-corpus id=T0382 category=jmespath-capture stage=execute expected=pass -->

```curl mdok name=source_11
curl "{{base_url}}/json/standard"
```

```jmespath mdok capture=source_11
{ids: body.items[].id}
```

```curl mdok name=use_11
curl "{{base_url}}/echo?case=capture-11"
```

```jmespath mdok check=use_11
status == `200`
```
