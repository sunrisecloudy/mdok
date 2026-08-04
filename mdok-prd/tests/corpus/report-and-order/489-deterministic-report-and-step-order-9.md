# T0489: deterministic report and step order 9

<!-- mdok-corpus id=T0489 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_8
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_8
status == `200`
```

```curl mdok name=second_8
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_8
status == `200`
```
