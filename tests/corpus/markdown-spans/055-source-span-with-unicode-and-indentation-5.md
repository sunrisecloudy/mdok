# T0055: source span with unicode and indentation 5

<!-- mdok-corpus id=T0055 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 4

> Documentation quote

```curl mdok name=unicode_4
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-4"
```

```jmespath mdok check=unicode_4
status == `200`
```
