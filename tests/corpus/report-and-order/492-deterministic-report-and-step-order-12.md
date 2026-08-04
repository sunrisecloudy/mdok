# T0492: deterministic report and step order 12

<!-- mdok-corpus id=T0492 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_11
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_11
status == `200`
```

```curl mdok name=second_11
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_11
status == `200`
```
