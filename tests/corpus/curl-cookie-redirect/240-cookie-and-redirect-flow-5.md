# T0240: cookie and redirect flow 5

<!-- mdok-corpus id=T0240 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_4
curl --cookie-jar "{{artifact_dir}}/cookie-4.txt" "{{base_url}}/cookies/set?name=c4&value=v4"
```

```jmespath mdok check=set_cookie_4
status == `200`
```

```curl mdok name=redirect_4
curl --location --max-redirs 5 --cookie "c4=v4" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_4
status == `200`
transfer.redirect_count == `2`
```
