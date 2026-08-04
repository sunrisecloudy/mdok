# T0072: single quoted literal header variant 12

<!-- mdok-corpus id=T0072 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_11
curl '{{base_url}}/echo' --header 'X-Test: one two'
```

```jmespath mdok check=shell_11
status == `200`
```
