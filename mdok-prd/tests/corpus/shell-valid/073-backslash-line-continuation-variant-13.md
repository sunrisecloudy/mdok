# T0073: backslash line continuation variant 13

<!-- mdok-corpus id=T0073 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_12
  curl "{{base_url}}/echo" \
--header "X-Test: continued"
  ```

  ```jmespath mdok check=shell_12
  status == `200`
  ```
