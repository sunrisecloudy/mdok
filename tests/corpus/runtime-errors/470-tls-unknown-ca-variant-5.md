# T0470: tls unknown ca variant 5

<!-- mdok-corpus id=T0470 category=runtime-errors stage=execute expected=error error=MDOK-E602 -->

```curl mdok name=rt_4
curl "{{https_base_url}}/health" --cacert {{wrong_ca_file}}
```
```jmespath mdok check=rt_4
status == `200`
```
