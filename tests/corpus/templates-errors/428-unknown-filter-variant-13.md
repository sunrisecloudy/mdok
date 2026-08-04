# T0428: unknown filter variant 13

<!-- mdok-corpus id=T0428 category=templates-errors stage=plan expected=error error=MDOK-E400 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_12
curl "{{base_url|magic}}/echo"
```
