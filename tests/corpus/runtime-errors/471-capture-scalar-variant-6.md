# T0471: capture scalar variant 6

<!-- mdok-corpus id=T0471 category=runtime-errors stage=execute expected=error error=MDOK-E503 -->

```curl mdok name=rt_5
curl "{{base_url}}/json/standard"
```
```jmespath mdok capture=rt_5
body.items[0].id
```
