# T0200: upload file variant 30

<!-- mdok-corpus id=T0200 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_29
curl "{{base_url}}/echo" --upload-file {{fixture_text_file}}
```

```jmespath mdok check=body_29
status == `200`
transfer.uploaded_bytes > `0`
```
