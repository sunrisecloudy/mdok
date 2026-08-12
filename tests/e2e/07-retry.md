# Retry policy

The local fixture returns two transient `503` responses for this test key and
then succeeds. The explicit key keeps mutable retry state isolated from other
workflows sharing the same fixture process.

```curl mdok name=retry
curl --retry 2 --retry-delay 0 \
  --header "X-Mdok-Test-Key: e2e-retry" \
  "{{base_url}}/retry/2"
```

```jmespath mdok check=retry
status == `200`
body.ok == `true`
body.attempt == `3`
```
