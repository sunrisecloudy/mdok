# T0178: multipart file variant 8

<!-- mdok-corpus id=T0178 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_7
curl "{{base_url}}/multipart" --form "file=@{{fixture_text_file}}"
```

```jmespath mdok check=body_7
status == `200`
body.multipart.files[0].size > `0`
```
