# T0239: cookie and redirect flow 4

<!-- mdok-corpus id=T0239 category=curl-cookie-redirect stage=execute expected=pass -->

```curl mdok name=set_cookie_3
curl --cookie-jar "{{artifact_dir}}/cookie-3.txt" "{{base_url}}/cookies/set?name=c3&value=v3"
```

```jmespath mdok check=set_cookie_3
status == `200`
```

```curl mdok name=redirect_3
curl --location --max-redirs 5 --cookie "c3=v3" "{{base_url}}/redirect/2?final=/cookies/echo"
```

```jmespath mdok check=redirect_3
status == `200`
transfer.redirect_count == `2`
```
