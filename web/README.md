# mdok promotional site

A dependency-free, static promotional page for mdok. It uses system fonts, inline
SVG, compact vanilla CSS, and a deferred vanilla JS file for only theme and copy
controls. There are no package dependencies, CDN requests, external assets, or
build step.

Files:

- `index.html` — four sections (hero, how it works, features, install), SEO metadata, JSON-LD, and inline favicon.
- `styles.css` — dark-first responsive styling with a light toggle; all colours are theme tokens on `:root`.
- `script.js` — optional copy buttons and theme persistence; the page works without it.
- `llms.txt` — quotable product context for language models.
- `robots.txt` / `sitemap.xml` — crawler instructions for the `mdok.dev` placeholder domain.

Serve locally from the repository root:

```sh
python3 -m http.server 8080 --directory web
```

Then open <http://127.0.0.1:8080/>. Opening `index.html` directly also works for
static content; a local server makes relative documentation links easier to test.
