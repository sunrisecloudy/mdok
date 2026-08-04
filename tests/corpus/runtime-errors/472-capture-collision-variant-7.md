# T0472: capture collision variant 7

<!-- mdok-corpus id=T0472 category=runtime-errors stage=execute expected=error error=MDOK-E504 -->

```curl mdok name=rt_6
curl "{{base_url}}/json/standard"
```
```jmespath mdok capture=rt_6
{x: body.items[0].id}
```
```jmespath mdok capture=rt_6
{x: body.items[1].id}
```
