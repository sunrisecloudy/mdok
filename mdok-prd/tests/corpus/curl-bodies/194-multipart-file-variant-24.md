# T0194: multipart file variant 24

<!-- mdok-corpus id=T0194 category=curl-bodies stage=execute expected=pass -->

```curl mdok name=body_23
curl "{{base_url}}/multipart" --form "file=@{{fixture_text_file}}"
```

```jmespath mdok check=body_23
status == `200`
body.multipart.files[0].size > `0`
```
