# T0060: source span with unicode and indentation 10

<!-- mdok-corpus id=T0060 category=markdown-spans stage=plan expected=pass -->

# ภาษาไทย 日本語 emoji 🚀 9

> Documentation quote

```curl mdok name=unicode_9
curl "{{base_url}}/echo?text=%E0%B9%84%E0%B8%97%E0%B8%A2-9"
```

```jmespath mdok check=unicode_9
status == `200`
```
