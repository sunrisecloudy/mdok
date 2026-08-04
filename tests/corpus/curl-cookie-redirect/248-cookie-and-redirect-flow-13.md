# T0248: cookie and redirect flow 13

<!-- mdok-corpus id=T0248 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_12
curl --cookie-jar "{{artifact_dir}}/cookie-12.txt" "{{base_url}}/cookies/set?name=c12&value=v12"
```

```jmespath mdok check=set_cookie_12
status == `200`
```

```curl mdok name=redirect_12
curl --location --max-redirs 5 --cookie "c12=v12" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_12
status == `200`
transfer.redirect_count == `2`
```
