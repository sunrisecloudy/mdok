# T0033: duplicate step names

<!-- mdok-corpus id=T0033 category=markdown-metadata stage=plan expected=error error=MDOK-E101 -->

```curl mdok name=x
curl "{{base_url}}/echo?a=1"
```

```curl mdok name=x
curl "{{base_url}}/echo?a=2"
```
