# Postman Collection Format reference (human-readable)

Field-level documentation of the Postman Collection Format. Two sources:

## 1. Archived from learning.postman.com (Wayback Machine)

The `postmanlabs/postman-docs` GitHub repo (learning.postman.com source) was
removed from GitHub and the rendered `learning.postman.com/collection-format/*`
pages are offline (all return 404 as of 2026-08-06). The Internet Archive
preserved a subset of pages; we archived the ones that survived:

| File | Original URL | Wayback snapshot |
| --- | --- | --- |
| `index.md` | https://learning.postman.com/collection-format/ | 20230308101958 |
| `getting-started/overview.md` | …/collection-format/getting-started/overview/ | 20230422213037 (page-data JSON) |
| `getting-started/structure-of-a-collection.md` | …/collection-format/getting-started/structure-of-a-collection/ | 20260107232532 (page-data JSON) |
| `advanced-concepts/events.md` | …/collection-format/advanced-concepts/events/ | 20230411163503 (page-data JSON) |
| `advanced-concepts/variables.md` | …/collection-format/advanced-concepts/variables/ | 20240421181709 (page-data JSON) |
| `reference/info.md` | …/collection-format/reference/info/ | 20250724002129 (page-data JSON) |
| `reference/request.md` | …/collection-format/reference/request/ | 20250504130838 (page-data JSON) |
| `reference/url.md` | …/collection-format/reference/url/ | 20230411164928 (page-data JSON) |

(The archived HTML pages are Gatsby client-rendered shells; the actual content
lives in the per-page `page-data/*/page-data.json` files, which is what we
saved and converted to Markdown.)

## 2. Generated from the canonical v2.1.0 schema

The remaining reference pages (`reference/reference/*.md`) were generated from
the vendored canonical schema `schemas/collection-v2.1.0.json` (the same
schemas the learning.postman.com pages were generated from). Each page
contains: the definition description, a property reference table (property /
type / description / enum), and the full schema JSON verbatim. These cover:
`collection`, `auth`, `auth-attribute`, `certificate(-list)`, `cookie(-list)`,
`description`, `event(-list)`, `header(-list)`, `item`, `item-group`,
`protocol-profile-behavior`, `proxy-config`, `response`, `script`,
`variable(-list)`, `version`.

> Note: `body` has no standalone definition in the schema — it is documented
> inside `reference/reference/request.md` (archived) and `reference/request.md`
> (archived page-data). `info`, `request`, `url` have archived pages and are
> *not* regenerated.
