# T0491: deterministic report and step order 11

<!-- mdok-corpus id=T0491 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_10
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_10
status == `200`
```

```curl mdok name=second_10
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_10
status == `200`
```
