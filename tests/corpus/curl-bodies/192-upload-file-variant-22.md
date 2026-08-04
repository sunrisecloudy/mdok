# T0192: upload file variant 22

<!-- mdok-corpus id=T0192 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_21
curl "{{base_url}}/echo" --upload-file {{fixture_text_file}}
```

```jmespath mdok check=body_21
status == `200`
transfer.uploaded_bytes > `0`
```
