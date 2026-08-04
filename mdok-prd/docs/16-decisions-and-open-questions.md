# 16. Decisions and Open Questions

## Final decisions

- Rust is the product/runtime language; C is limited to curl integration.
- Comrak parses Markdown.
- Tree-sitter Bash parses curl fence shell structure.
- curl's actual tool parser interprets curl options.
- libcurl performs transfers.
- Standard JMESPath handles checks and captures.
- TOML handles project and inline variable tables.
- Version 1 executes one transfer per curl fence and no arbitrary shell.
- Version 1 is sequential within one document.

## Open questions requiring implementation spikes

1. How small can the curl-tool patch remain while exposing a stable parse plan?
2. Should the bridge execute from curl's internal `OperationConfig` directly, or convert to an MDOK-owned plan before execution?
3. Which TLS backend provides the best release portability on macOS without surprising trust-store behavior?
4. How should huge JSON bodies be queried without violating memory bounds?
5. Is conservative secret taint practical through the selected Rust JMESPath implementation, or should redaction use value fingerprinting plus path policy in version 1?
6. Should `ws`/`wss` be version 1 or a later feature?
7. Should explicitly referenced curl config files be supported in version 1 or rejected until policy handling is mature?

None of these questions changes the user-facing core language. Phase 0 should resolve the curl bridge questions before broad implementation.
