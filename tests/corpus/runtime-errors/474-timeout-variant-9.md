# T0474: timeout variant 9

<!-- mdok-corpus id=T0474 category=runtime-errors stage=execute expected=error error=MDOK-E601 -->

```curl mdok name=rt_8
curl --max-time 0.01 "{{base_url}}/delay/200"
```
```jmespath mdok check=rt_8
status == `200`
```
