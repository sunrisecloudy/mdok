# T0242: cookie and redirect flow 7

<!-- mdok-corpus id=T0242 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_6
curl --cookie-jar "{{artifact_dir}}/cookie-6.txt" "{{base_url}}/cookies/set?name=c6&value=v6"
```

```jmespath mdok check=set_cookie_6
status == `200`
```

```curl mdok name=redirect_6
curl --location --max-redirs 5 --cookie "c6=v6" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_6
status == `200`
transfer.redirect_count == `2`
```
