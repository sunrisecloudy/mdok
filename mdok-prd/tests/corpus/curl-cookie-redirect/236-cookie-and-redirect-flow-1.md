# T0236: cookie and redirect flow 1

<!-- mdok-corpus id=T0236 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_0
curl --cookie-jar "{{artifact_dir}}/cookie-0.txt" "{{base_url}}/cookies/set?name=c0&value=v0"
```

```jmespath mdok check=set_cookie_0
status == `200`
```

```curl mdok name=redirect_0
curl --location --max-redirs 5 --cookie "c0=v0" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_0
status == `200`
transfer.redirect_count == `2`
```
