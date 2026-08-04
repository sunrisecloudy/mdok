# T0054: source span with unicode and indentation 4

<!-- mdok-corpus id=T0054 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 3

> Documentation quote

```curl mdok name=unicode_3
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-3"
```

```jmespath mdok check=unicode_3
status == `200`
```
