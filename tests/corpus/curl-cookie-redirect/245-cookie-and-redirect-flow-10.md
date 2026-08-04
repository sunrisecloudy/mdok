# T0245: cookie and redirect flow 10

<!-- mdok-corpus id=T0245 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_9
curl --cookie-jar "{{artifact_dir}}/cookie-9.txt" "{{base_url}}/cookies/set?name=c9&value=v9"
```

```jmespath mdok check=set_cookie_9
status == `200`
```

```curl mdok name=redirect_9
curl --location --max-redirs 5 --cookie "c9=v9" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_9
status == `200`
transfer.redirect_count == `2`
```
