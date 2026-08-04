# T0184: upload file variant 14

<!-- mdok-corpus id=T0184 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_13
curl "{{base_url}}/echo" --upload-file {{fixture_text_file}}
```

```jmespath mdok check=body_13
status == `200`
transfer.uploaded_bytes > `0`
```
