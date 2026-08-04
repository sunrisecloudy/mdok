# T0241: cookie and redirect flow 6

<!-- mdok-corpus id=T0241 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_5
curl --cookie-jar "{{artifact_dir}}/cookie-5.txt" "{{base_url}}/cookies/set?name=c5&value=v5"
```

```jmespath mdok check=set_cookie_5
status == `200`
```

```curl mdok name=redirect_5
curl --location --max-redirs 5 --cookie "c5=v5" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_5
status == `200`
transfer.redirect_count == `2`
```
