# T0052: source span with unicode and indentation 2

<!-- mdok-corpus id=T0052 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 1

> Documentation quote

```curl mdok name=unicode_1
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-1"
```

```jmespath mdok check=unicode_1
status == `200`
```
