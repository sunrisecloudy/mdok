# T0183: binary file upload variant 13

<!-- mdok-corpus id=T0183 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_12
curl "{{base_url}}/echo" --data-binary @{{fixture_text_file}}
```

```jmespath mdok check=body_12
status == `200`
transfer.uploaded_bytes > `0`
```
