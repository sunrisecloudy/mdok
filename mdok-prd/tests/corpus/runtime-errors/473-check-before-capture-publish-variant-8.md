# T0473: check before capture publish variant 8

<!-- mdok-corpus id=T0473 category=runtime-errors stage=execute expected=error error=MDOK-E502 -->

```curl mdok name=rt_7
curl "{{base_url}}/status/500"
```
```jmespath mdok check=rt_7
status == `200`
```
```jmespath mdok capture=rt_7
{x: body.value}
```
