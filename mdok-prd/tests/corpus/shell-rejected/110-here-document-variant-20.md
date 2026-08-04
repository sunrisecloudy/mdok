# T0110: here document variant 20

<!-- mdok-corpus id=T0110 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_19
    curl --data-binary @- "{{base_url}}/echo" <<EOF
x
EOF
    ```
