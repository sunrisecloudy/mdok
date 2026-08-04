# T0130: here document variant 40

<!-- mdok-corpus id=T0130 category=shell-rejected stage=plan expected=error error=MDOK-E201 -->

```curl mdok name=bad_39
    curl --data-binary @- "{{base_url}}/echo" <<EOF
x
EOF
    ```
