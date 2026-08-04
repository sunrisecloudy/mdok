# T0476: redirect limit variant 11

<!-- mdok-corpus id=T0476 category=runtime-errors stage=execute expected=error error=MDOK-E603 -->

```curl mdok name=rt_10
curl --location --max-redirs 1 "{{base_url}}/redirect/3"
```
```jmespath mdok check=rt_10
status == `200`
```
