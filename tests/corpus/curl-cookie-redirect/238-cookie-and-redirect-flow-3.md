# T0238: cookie and redirect flow 3

<!-- mdok-corpus id=T0238 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_2
curl --cookie-jar "{{artifact_dir}}/cookie-2.txt" "{{base_url}}/cookies/set?name=c2&value=v2"
```

```jmespath mdok check=set_cookie_2
status == `200`
```

```curl mdok name=redirect_2
curl --location --max-redirs 5 --cookie "c2=v2" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_2
status == `200`
transfer.redirect_count == `2`
```
