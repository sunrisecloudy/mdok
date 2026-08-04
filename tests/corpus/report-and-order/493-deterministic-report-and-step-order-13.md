# T0493: deterministic report and step order 13

<!-- mdok-corpus id=T0493 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_12
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_12
status == `200`
```

```curl mdok name=second_12
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_12
status == `200`
```
