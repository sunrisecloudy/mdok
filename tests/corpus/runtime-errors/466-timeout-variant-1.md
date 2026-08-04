# T0466: timeout variant 1

<!-- mdok-corpus id=T0466 category=runtime-errors stage=execute expected=error error=MDOK-E601 -->

```curl mdok name=rt_0
curl --max-time 0.01 "{{base_url}}/delay/200"
```
```jmespath mdok check=rt_0
status == `200`
```
