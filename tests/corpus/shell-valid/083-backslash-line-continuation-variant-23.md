# T0083: backslash line continuation variant 23

<!-- mdok-corpus id=T0083 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_22
  curl "{{base_url}}/echo" \
--header "X-Test: continued"
  ```

  ```jmespath mdok check=shell_22
  status == `200`
  ```
