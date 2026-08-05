# Stored agent command

This document is a durable command test that an agent can keep, review, and
run again as the repository changes.

```exec mdok name=agent_message
mdok-command-fixture json
```

```jmespath mdok check=agent_message
success == `true`
```

```jmespath mdok check=agent_message
output_truncated == `false`
```

```jmespath mdok check=agent_message
stdout_json.ok == `true`
```

```exec mdok name=agent_argv
mdok-command-fixture echo "agent value" literal
```

```jmespath mdok check=agent_argv
contains(stdout, 'agent value') == `true`
```
