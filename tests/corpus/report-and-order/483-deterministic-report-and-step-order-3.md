# T0483: deterministic report and step order 3

<!-- mdok-corpus id=T0483 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_2
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_2
status == `200`
```

```curl mdok name=second_2
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_2
status == `200`
```
