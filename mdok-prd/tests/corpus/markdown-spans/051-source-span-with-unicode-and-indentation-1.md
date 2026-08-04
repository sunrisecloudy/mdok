# T0051: source span with unicode and indentation 1

<!-- mdok-corpus id=T0051 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 0

> Documentation quote

```curl mdok name=unicode_0
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-0"
```

```jmespath mdok check=unicode_0
status == `200`
```
