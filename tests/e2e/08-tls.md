# Verified local TLS

The E2E runner supplies `https_base_url` and the test server's generated
`ca_file`. This verifies the local certificate chain without `--insecure`.

```curl mdok name=verified_tls
curl "{{https_base_url}}/health" --cacert "{{ca_file}}"
```

```jmespath mdok check=verified_tls
status == `200`
body.ok == `true`
```
