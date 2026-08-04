# ADR 0005: No Arbitrary Shell Execution in Version 1

Status: Accepted

A `curl` fence describes one curl command but never invokes a shell. Pipelines, substitutions, redirects, and extra programs are rejected. This improves portability, determinism, and security and keeps JMESPath as the response-processing language.
