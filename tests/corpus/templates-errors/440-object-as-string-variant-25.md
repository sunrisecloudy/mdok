# T0440: object as string variant 25

<!-- mdok-corpus id=T0440 category=templates-errors stage=plan expected=error error=MDOK-E402 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_24
curl "{{object_value}}"
```
