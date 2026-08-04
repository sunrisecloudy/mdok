# T0059: source span with unicode and indentation 9

<!-- mdok-corpus id=T0059 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 8

> Documentation quote

```curl mdok name=unicode_8
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-8"
```

```jmespath mdok check=unicode_8
status == `200`
```
