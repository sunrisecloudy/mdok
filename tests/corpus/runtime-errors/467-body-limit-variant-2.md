# T0467: body limit variant 2

<!-- mdok-corpus id=T0467 category=runtime-errors stage=execute expected=error error=MDOK-E700 -->

```curl mdok name=rt_1
curl "{{base_url}}/large/1048576"
```
```jmespath mdok check=rt_1
status == `200`
```
