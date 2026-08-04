# T0490: deterministic report and step order 10

<!-- mdok-corpus id=T0490 category=report-and-order stage=report expected=pass -->

```curl mdok name=first_9
curl "{{base_url}}/echo?step=first"
```
```jmespath mdok check=first_9
status == `200`
```

```curl mdok name=second_9
curl "{{base_url}}/echo?step=second"
```
```jmespath mdok check=second_9
status == `200`
```
