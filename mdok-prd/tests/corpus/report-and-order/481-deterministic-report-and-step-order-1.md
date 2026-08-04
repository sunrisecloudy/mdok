# T0481: deterministic report and step order 1

<!-- mdok-corpus id=T0481 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_0
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_0
status == `200`
```

```curl mdok name=second_0
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_0
status == `200`
```
