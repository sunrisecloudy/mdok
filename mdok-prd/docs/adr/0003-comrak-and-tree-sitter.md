# ADR 0003: Comrak for Markdown and Tree-sitter Bash for curl Fences

Status: Accepted

Comrak provides a full CommonMark/GFM AST suitable for structure and source locations. Tree-sitter Bash validates that a curl fence is one non-executable simple command and allows precise rejection of shell constructs. Regex and naive line splitting are prohibited.
