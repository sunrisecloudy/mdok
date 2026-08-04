# T0475: body limit variant 10

<!-- mdok-corpus id=T0475 category=runtime-errors stage=execute expected=error error=MDOK-E700 -->

```curl mdok name=rt_9
curl "{{base_url}}/large/1048576"
```
```jmespath mdok check=rt_9
status == `200`
```
