# T0477: early close variant 12

<!-- mdok-corpus id=T0477 category=runtime-errors stage=execute expected=error error=MDOK-E600 -->

```curl mdok name=rt_11
curl "{{base_url}}/close/early"
```
```jmespath mdok check=rt_11
status == `200`
```
