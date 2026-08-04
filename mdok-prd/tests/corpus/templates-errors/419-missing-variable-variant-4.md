# T0419: missing variable variant 4

<!-- mdok-corpus id=T0419 category=templates-errors stage=plan expected=error error=MDOK-E401 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_3
curl "{{missing}}/echo"
```
