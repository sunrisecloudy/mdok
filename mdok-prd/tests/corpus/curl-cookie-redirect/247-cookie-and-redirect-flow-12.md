# T0247: cookie and redirect flow 12

<!-- mdok-corpus id=T0247 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_11
curl --cookie-jar "{{artifact_dir}}/cookie-11.txt" "{{base_url}}/cookies/set?name=c11&value=v11"
```

```jmespath mdok check=set_cookie_11
status == `200`
```

```curl mdok name=redirect_11
curl --location --max-redirs 5 --cookie "c11=v11" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_11
status == `200`
transfer.redirect_count == `2`
```
