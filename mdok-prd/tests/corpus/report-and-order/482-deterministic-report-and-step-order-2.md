# T0482: deterministic report and step order 2

<!-- mdok-corpus id=T0482 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_1
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_1
status == `200`
```

```curl mdok name=second_1
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_1
status == `200`
```
