# T0199: binary file upload variant 29

<!-- mdok-corpus id=T0199 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_28
curl "{{base_url}}/echo" --data-binary @{{fixture_text_file}}
```

```jmespath mdok check=body_28
status == `200`
transfer.uploaded_bytes > `0`
```
