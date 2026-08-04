# T0431: array as header variant 16

<!-- mdok-corpus id=T0431 category=templates-errors stage=plan expected=error error=MDOK-E402 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_15
curl "{{base_url}}/echo" -H "X: {{array_value|header}}"
```
