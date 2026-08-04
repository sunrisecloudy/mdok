# T0063: backslash line continuation variant 3

<!-- mdok-corpus id=T0063 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_2
  curl "{{base_url}}/echo" \
--header "X-Test: continued"
  ```

  ```jmespath mdok check=shell_2
  status == `200`
  ```
