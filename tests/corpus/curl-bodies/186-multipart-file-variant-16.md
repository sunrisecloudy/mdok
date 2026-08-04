# T0186: multipart file variant 16

<!-- mdok-corpus id=T0186 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_15
curl "{{base_url}}/multipart" --form "file=@{{fixture_text_file}}"
```

```jmespath mdok check=body_15
status == `200`
body.multipart.files[0].size > `0`
```
