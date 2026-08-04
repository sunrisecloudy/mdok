# T0237: cookie and redirect flow 2

<!-- mdok-corpus id=T0237 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_1
curl --cookie-jar "{{artifact_dir}}/cookie-1.txt" "{{base_url}}/cookies/set?name=c1&value=v1"
```

```jmespath mdok check=set_cookie_1
status == `200`
```

```curl mdok name=redirect_1
curl --location --max-redirs 5 --cookie "c1=v1" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_1
status == `200`
transfer.redirect_count == `2`
```
