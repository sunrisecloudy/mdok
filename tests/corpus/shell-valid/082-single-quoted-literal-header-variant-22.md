# T0082: single quoted literal header variant 22

<!-- mdok-corpus id=T0082 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_21
curl '{{base_url}}/echo' --header 'X-Test: one two'
```

```jmespath mdok check=shell_21
status == `200`
```
