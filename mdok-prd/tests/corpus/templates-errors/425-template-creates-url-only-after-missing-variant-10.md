# T0425: template creates url only after missing variant 10

<!-- mdok-corpus id=T0425 category=templates-errors stage=plan expected=error error=MDOK-E401 -->

```toml mdok vars
object_value = { a = 1 }
array_value = [1, 2]
newline_value = "line1\nline2"
```


```curl mdok name=template_bad_9
curl "{{missing|url}}"
```
