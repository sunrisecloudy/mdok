# T0176: upload file variant 6

<!-- mdok-corpus id=T0176 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_5
curl "{{base_url}}/echo" --upload-file {{fixture_text_file}}
```

```jmespath mdok check=body_5
status == `200`
transfer.uploaded_bytes > `0`
```
