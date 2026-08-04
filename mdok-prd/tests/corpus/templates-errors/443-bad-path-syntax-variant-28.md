# T0443: bad path syntax variant 28

<!-- mdok-corpus id=T0443 category=templates-errors stage=plan expected=error error=MDOK-E400 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_27
curl "{{user..id}}"
```
