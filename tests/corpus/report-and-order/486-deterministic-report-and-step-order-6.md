# T0486: deterministic report and step order 6

<!-- mdok-corpus id=T0486 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_5
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_5
status == `200`
```

```curl mdok name=second_5
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_5
status == `200`
```
