# Health workflow

```toml mdok vars
base_url = "https://api.example.test"
```

## Status

```curl mdok name=health
curl "{{base_url}}/health"
```

```jmespath mdok check=health
status == `200`
```
