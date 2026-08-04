# T0056: source span with unicode and indentation 6

<!-- mdok-corpus id=T0056 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 5

> Documentation quote

```curl mdok name=unicode_5
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-5"
```

```jmespath mdok check=unicode_5
status == `200`
```
