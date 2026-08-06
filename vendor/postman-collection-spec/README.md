# Postman Collection Format specification (vendored reference)

Reference material for MDOK's Postman compatibility work. Like `vendor/celld`,
this is an upstream oracle: MDOK does not link, embed, or execute it; the
schemas and docs are used by `scripts/check_postman_spec_coverage.py` to
enumerate the declarable Postman surface and verify that the mdok-postman
importer and mdok-quickjs `pm` facade either support every element or fail
with a named compatibility diagnostic (see `docs/CELLD_QUICKJS_ADAPTATION.md`
and `docs/POSTMAN_SPEC_COVERAGE.md`).

## Files

- `schemas/collection-v2.1.0.json` — official Postman Collection JSON Schema
  v2.1.0 (the canonical spec document; bytes fetched verbatim from
  `https://schema.getpostman.com/json/collection/v2.1.0/collection.json`).
- `schemas/collection-v2.0.0.json` — v2.0.0 schema for reference (same source
  URL pattern).
- `schemas/json-schema-draft-07.json` — JSON Schema draft-07 metaschema
  (fetched from `http://json-schema.org/draft-07/schema`) for standalone
  validation of the collection schema.
- `upstream/` — provenance from the schema source repository
  `https://github.com/postmanlabs/schemas`:
  - `schemas-repo-commit.txt` — upstream commit SHA
    (`e462f6bf344efcff1360710de4e48741ea0df941`).
  - `schemas-README.md`, `schemas-CHANGELOG.yaml` — upstream README/CHANGELOG.
  - `LICENSE-postmanlabs-schemas.md` — upstream license (Apache-2.0).
- `docs/` (optional) — human-readable Postman documentation pages if fetched
  (collection format reference / Postman sandbox API reference from
  `https://github.com/postmanlabs/postman-docs`).

Retrieved: 2026-08-06. License: Apache-2.0 (postmanlabs/schemas).

## How it is used

`python3 scripts/check_postman_spec_coverage.py` walks
`schemas/collection-v2.1.0.json`, enumerates every declarable element
(properties, auth types, body modes, event listeners, ...), and classifies
each against the mdok-postman importer (supported / named diagnostic) and the
mdok-quickjs `pm` facade (`mdok-pm-probe --list-api`). The gate passes only
when the `missing` set is empty.
