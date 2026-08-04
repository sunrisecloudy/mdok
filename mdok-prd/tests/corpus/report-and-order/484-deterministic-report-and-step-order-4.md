# T0484: deterministic report and step order 4

<!-- mdok-corpus id=T0484 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_3
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_3
status == `200`
```

```curl mdok name=second_3
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_3
status == `200`
```
