# T0487: deterministic report and step order 7

<!-- mdok-corpus id=T0487 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_6
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_6
status == `200`
```

```curl mdok name=second_6
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_6
status == `200`
```
