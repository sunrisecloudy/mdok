# T0488: deterministic report and step order 8

<!-- mdok-corpus id=T0488 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_7
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_7
status == `200`
```

```curl mdok name=second_7
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_7
status == `200`
```
