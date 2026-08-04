# T0480: capture collision variant 15

<!-- mdok-corpus id=T0480 category=runtime-errors stage=execute expected=error error=MDOK-E504 -->

```curl mdok name=rt_14
curl "{{base_url}}/json/standard"
```
```jmespath mdok capture=rt_14
{x: body.items[0].id}
```
```jmespath mdok capture=rt_14
{x: body.items[1].id}
```
