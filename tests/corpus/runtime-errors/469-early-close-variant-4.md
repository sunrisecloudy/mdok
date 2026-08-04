# T0469: early close variant 4

<!-- mdok-corpus id=T0469 category=runtime-errors stage=execute expected=error error=MDOK-E600 -->

```curl mdok name=rt_3
curl "{{base_url}}/close/early"
```
```jmespath mdok check=rt_3
status == `200`
```
