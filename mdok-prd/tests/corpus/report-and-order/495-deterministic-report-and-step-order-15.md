# T0495: deterministic report and step order 15

<!-- mdok-corpus id=T0495 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_14
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_14
status == `200`
```

```curl mdok name=second_14
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_14
status == `200`
```
