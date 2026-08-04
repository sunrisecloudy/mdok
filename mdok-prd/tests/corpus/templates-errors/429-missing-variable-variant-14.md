# T0429: missing variable variant 14

<!-- mdok-corpus id=T0429 category=templates-errors stage=plan expected=error error=MDOK-E401 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_13
curl "{{missing}}/echo"
```
