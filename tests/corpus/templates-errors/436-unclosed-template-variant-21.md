# T0436: unclosed template variant 21

<!-- mdok-corpus id=T0436 category=templates-errors stage=plan expected=error error=MDOK-E400 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_20
curl "{{base_url}/echo"
```
