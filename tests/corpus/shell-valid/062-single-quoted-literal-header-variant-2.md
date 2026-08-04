# T0062: single quoted literal header variant 2

<!-- mdok-corpus id=T0062 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_1
curl '{{base_url}}/echo' --header 'X-Test: one two'
```

```jmespath mdok check=shell_1
status == `200`
```
