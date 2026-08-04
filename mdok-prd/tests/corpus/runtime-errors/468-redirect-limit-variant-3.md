# T0468: redirect limit variant 3

<!-- mdok-corpus id=T0468 category=runtime-errors stage=execute expected=error error=MDOK-E603 -->

```curl mdok name=rt_2
curl --location --max-redirs 1 "{{base_url}}/redirect/3"
```
```jmespath mdok check=rt_2
status == `200`
```
