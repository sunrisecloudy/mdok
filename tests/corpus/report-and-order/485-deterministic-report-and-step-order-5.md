# T0485: deterministic report and step order 5

<!-- mdok-corpus id=T0485 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_4
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_4
status == `200`
```

```curl mdok name=second_4
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_4
status == `200`
```
