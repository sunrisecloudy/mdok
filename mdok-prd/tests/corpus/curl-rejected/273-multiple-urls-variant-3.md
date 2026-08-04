# T0273: multiple urls variant 3

<!-- mdok-corpus id=T0273 category=curl-rejected stage=plan expected=error error=MDOK-E304 -->

```curl mdok name=rejected_2
curl "{{base_url}}/echo" "{{base_url}}/echo"
```
