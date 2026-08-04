# T0070: repeated headers variant 10

<!-- mdok-corpus id=T0070 category=shell-valid stage=execute expected=pass -->

```curl mdok name=shell_9
curl "{{base_url}}/echo" -H "X-A: 1" -H "X-A: 2"
```

```jmespath mdok check=shell_9
status == `200`
```
