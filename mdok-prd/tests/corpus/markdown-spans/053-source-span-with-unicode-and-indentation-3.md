# T0053: source span with unicode and indentation 3

<!-- mdok-corpus id=T0053 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 2

> Documentation quote

```curl mdok name=unicode_2
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-2"
```

```jmespath mdok check=unicode_2
status == `200`
```
