# T0175: binary file upload variant 5

<!-- mdok-corpus id=T0175 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_4
curl "{{base_url}}/echo" --data-binary @{{fixture_text_file}}
```

```jmespath mdok check=body_4
status == `200`
transfer.uploaded_bytes > `0`
```
