# T0497: template path depth limit

<!-- mdok-corpus id=T0497 category=templates-errors stage=plan expected=error -->

```curl mdok name=step_0
curl -d "{{a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a.a}}" "https://api.example.test/x"
```
