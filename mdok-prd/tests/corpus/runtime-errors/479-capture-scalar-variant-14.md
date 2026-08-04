# T0479: capture scalar variant 14

<!-- mdok-corpus id=T0479 category=runtime-errors stage=execute expected=error error=MDOK-E503 -->

```curl mdok name=rt_13
curl "{{base_url}}/json/standard"
```
```jmespath mdok capture=rt_13
body.items[0].id
```
