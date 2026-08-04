# T0191: binary file upload variant 21

<!-- mdok-corpus id=T0191 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_20
curl "{{base_url}}/echo" --data-binary @{{fixture_text_file}}
```

```jmespath mdok check=body_20
status == `200`
transfer.uploaded_bytes > `0`
```
