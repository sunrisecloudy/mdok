# T0250: cookie and redirect flow 15

<!-- mdok-corpus id=T0250 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_14
curl --cookie-jar "{{artifact_dir}}/cookie-14.txt" "{{base_url}}/cookies/set?name=c14&value=v14"
```

```jmespath mdok check=set_cookie_14
status == `200`
```

```curl mdok name=redirect_14
curl --location --max-redirs 5 --cookie "c14=v14" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_14
status == `200`
transfer.redirect_count == `2`
```
