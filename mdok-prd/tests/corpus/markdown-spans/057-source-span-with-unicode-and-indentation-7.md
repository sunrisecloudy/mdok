# T0057: source span with unicode and indentation 7

<!-- mdok-corpus id=T0057 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 6

> Documentation quote

```curl mdok name=unicode_6
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-6"
```

```jmespath mdok check=unicode_6
status == `200`
```
