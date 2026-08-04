# T0478: tls unknown ca variant 13

<!-- mdok-corpus id=T0478 category=runtime-errors stage=execute expected=error error=MDOK-E602 -->

```curl mdok name=rt_12
curl "{{https_base_url}}/health" --cacert {{wrong_ca_file}}
```
```jmespath mdok check=rt_12
status == `200`
```
