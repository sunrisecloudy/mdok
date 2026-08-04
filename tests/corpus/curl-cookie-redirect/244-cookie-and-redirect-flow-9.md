# T0244: cookie and redirect flow 9

<!-- mdok-corpus id=T0244 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_8
curl --cookie-jar "{{artifact_dir}}/cookie-8.txt" "{{base_url}}/cookies/set?name=c8&value=v8"
```

```jmespath mdok check=set_cookie_8
status == `200`
```

```curl mdok name=redirect_8
curl --location --max-redirs 5 --cookie "c8=v8" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_8
status == `200`
transfer.redirect_count == `2`
```
