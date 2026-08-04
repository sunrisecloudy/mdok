# T0249: cookie and redirect flow 14

<!-- mdok-corpus id=T0249 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_13
curl --cookie-jar "{{artifact_dir}}/cookie-13.txt" "{{base_url}}/cookies/set?name=c13&value=v13"
```

```jmespath mdok check=set_cookie_13
status == `200`
```

```curl mdok name=redirect_13
curl --location --max-redirs 5 --cookie "c13=v13" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_13
status == `200`
transfer.redirect_count == `2`
```
