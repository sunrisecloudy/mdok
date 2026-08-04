# T0243: cookie and redirect flow 8

<!-- mdok-corpus id=T0243 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_7
curl --cookie-jar "{{artifact_dir}}/cookie-7.txt" "{{base_url}}/cookies/set?name=c7&value=v7"
```

```jmespath mdok check=set_cookie_7
status == `200`
```

```curl mdok name=redirect_7
curl --location --max-redirs 5 --cookie "c7=v7" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_7
status == `200`
transfer.redirect_count == `2`
```
