# T0246: cookie and redirect flow 11

<!-- mdok-corpus id=T0246 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_10
curl --cookie-jar "{{artifact_dir}}/cookie-10.txt" "{{base_url}}/cookies/set?name=c10&value=v10"
```

```jmespath mdok check=set_cookie_10
status == `200`
```

```curl mdok name=redirect_10
curl --location --max-redirs 5 --cookie "c10=v10" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_10
status == `200`
transfer.redirect_count == `2`
```
