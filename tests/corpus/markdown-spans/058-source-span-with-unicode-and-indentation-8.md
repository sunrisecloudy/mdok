# T0058: source span with unicode and indentation 8

<!-- mdok-corpus id=T0058 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 7

> Documentation quote

```curl mdok name=unicode_7
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-7"
```

```jmespath mdok check=unicode_7
status == `200`
```
