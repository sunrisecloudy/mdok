# T0426: unclosed template variant 11

<!-- mdok-corpus id=T0426 category=templates-errors stage=plan expected=error error=MDOK-E400 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_10
curl "{{base_url}/echo"
```
