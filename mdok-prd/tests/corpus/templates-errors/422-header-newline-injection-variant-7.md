# T0422: header newline injection variant 7

<!-- mdok-corpus id=T0422 category=templates-errors stage=execute expected=error error=MDOK-E403 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_6
curl "{{base_url}}/echo" -H "X: {{newline_value|header}}"
```
