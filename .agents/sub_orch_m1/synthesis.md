# Milestone 1 Synthesis: Layout, SEO, Navigation, and CSS Cleaning

## Consensus
All three Explorer agents agree on the exact requirements and target files for Milestone 1:

### 1. SiteLayout.astro Modifications
- **Skip Link**: Add `<a href="#main-content" class="skip-link">Skip to content</a>` at the beginning of `<body>`. Wrap the main page content in `<main id="main-content" tabindex="-1">`. Add focus styles in CSS.
- **SEO & Meta**: Add Open Graph and Twitter card meta tags to `<head>`.
- **Favicon**: Add a standard `<link rel="icon" type="image/svg+xml" href={`${baseUrl}favicon.svg`} />` (or equivalent).
- **Fonts**: Load the Inter font from Google Fonts.
- **Color Scheme**: Set `<meta name="color-scheme" content="dark" />` instead of `dark light`.
- **Footer**: Add a footer with copyright, repository link (`https://github.com/projectbluefin/lab`), and a build timestamp.
- **Navigation**:
  - Add "Overview" pointing to `/`.
  - Add "Bluefin" pointing to `${baseUrl}bluefin/` and "About" pointing to `${baseUrl}about/`.
  - Rewrite navigation active state highlighting using `Astro.url.pathname` to ensure proper highlighting for Overview, `/bluefin/`, and `/about/`.

### 2. Bluefin Page (src/pages/bluefin.astro)
- Add a header block with `<h1>Bluefin Upstream Status</h1>` to ensure exactly one `<h1>`.
- Set `current="bluefin"` (or use the pathname-based navigation matching).

### 3. About Page Migration
- Convert `docs/about/methodology.html` to `src/pages/about.astro` using `SiteLayout` and passing `current="about"`.
- Clean up footer links:
  - Repository: `https://github.com/projectbluefin/lab`
  - Schemas: `https://github.com/projectbluefin/lab/tree/main/schemas/v2`
  - Remove dead `plan.md` link or point to `AGENTS.md` (remove is preferred since `plan.md` is internal).

### 4. Stylesheet (src/styles/site.css)
- Move `.pill--failed` out of the amber group and style it red:
  ```css
  .pill--failed {
    background: rgba(251, 113, 133, 0.16);
    color: #fb7185;
  }
  ```
- Add custom semantic status variables in `:root`:
  ```css
  :root {
    ...
    --status-passed: #4ade80; /* green */
    --status-failed: #fb7185; /* red */
    --status-pending: #fbbf24; /* yellow */
    --status-unavailable: #94a3b8; /* slate */
  }
  ```
- Remove dead CSS selectors: `.status-grid` and `.status-card--muted`.
- Delete dead prototype/unused files:
  - `src/components/UnavailablePanel.astro` (unused component)
  - `docs/prototype-factory.html` (legacy prototype)
  - `flatcar-clone-prototype.py` (legacy prototype)

### 5. Build & Test Updates
- Update the clean build script in `package.json` to also clear `docs/about` when running `npm run build`.
- Update `tests/astro-foundation.test.mjs`:
  - Add `docs/about/index.html` to `expectedFiles` array.
  - Assert that `docs/about/index.html` contains "Bluefin QA — Methodology".
  - Remove/adjust the assertion: `assert.doesNotMatch(html('docs/index.html'), /site-nav__link[^>]*>Overview</)` since "Overview" is now explicitly added to navigation.

## Gaps
None. All requirements in Milestone 1 are fully covered.
