# T0494: deterministic report and step order 14

<!-- mdok-corpus id=T0494 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_13
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_13
status == `200`
```

```curl mdok name=second_13
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_13
status == `200`
```
