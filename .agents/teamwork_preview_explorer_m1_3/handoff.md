# Handoff Report: Milestone 1 Layout, SEO, Navigation, and CSS Cleaning

## 1. Observation

### SiteLayout.astro Changes Needed
In `src/layouts/SiteLayout.astro`, we observed:
* **Skip Link**: No skip link exists in the `<body>` (lines 38–57).
* **Footer**: No footer elements exist at the bottom of the body.
* **Favicon**: No favicon links exist in the `<head>` (lines 25–37).
* **Open Graph / Twitter Meta Tags**: Standard SEO meta tags are missing. Only `<meta name="description" />` is defined (line 28).
* **Inter Font Load**: No external font links or local `@font-face` styles exist for the Inter font.
* **Dark Color-Scheme**: Line 29 has `<meta name="color-scheme" content="dark light" />`, which enables both dark and light modes, but does not enforce/support dark-only styles natively at the document level.
* **Active Navigation Logic**:
  * Line 7: `current` type is defined as: `current: 'overview' | 'upstream' | 'tests' | 'applications' | 'homebrew' | 'adoption' | 'userspace';`
  * Lines 13–20 define `navItems`:
    ```javascript
    const navItems = [
      { id: 'upstream', label: 'Upstream', href: `${baseUrl}upstream/` },
      { id: 'tests', label: 'Tests', href: `${baseUrl}tests/` },
      { id: 'applications', label: 'Applications', href: `${baseUrl}applications/` },
      { id: 'homebrew', label: 'Homebrew', href: `${baseUrl}homebrew/` },
      { id: 'adoption', label: 'Adoption', href: `${baseUrl}adoption/` },
      { id: 'userspace', label: 'Userspace', href: `${baseUrl}userspace/` },
    ] as const;
    ```

### Bluefin Page (`src/pages/bluefin.astro`) Inspection
In `src/pages/bluefin.astro`, we observed:
* **`h1` Heading Count**: There are **zero `<h1>` tags** on the entire page. Only `<h2>`, `<h3>`, and `<h4>` are used.
* **Navigation Highlights**:
  * Line 18 defines the layout properties:
    ```astro
    <SiteLayout
      title="Bluefin upstream"
      description="Factory upstream status focused on Bluefin, Bluefin-LTS, and Dakota image streams."
      current="upstream"
    >
    ```
    This highlights "Upstream" rather than "Bluefin".

### Methodology Page (`docs/about/methodology.html`) Inspection
We located the file at `docs/about/methodology.html`. It contains:
* Custom CSS variables inside a block (lines 8–27).
* An inline header element:
  ```html
  <header>
    <h1>Bluefin QA — Methodology</h1>
    <div class="header-meta"><a href="/">&larr; back to dashboard</a></div>
  </header>
  ```
* Sections for **Derived metrics** (with a `<table>`), **Failure Mode and Effects Analysis (FMEA)**, and **State vocabulary**.
* A footer on lines 211–215 with broken links:
  ```html
  <footer>
    <a href="https://github.com/castrojo/testing-lab">repository</a>
    <a href="https://github.com/castrojo/testing-lab/blob/main/plan.md">plan.md</a>
    <a href="https://github.com/castrojo/testing-lab/tree/main/schemas/v2">schemas/v2/</a>
  </footer>
  ```
* These link to the old repository `castrojo/testing-lab` (which is now `projectbluefin/lab`), and `plan.md` does not exist at the root of the new repository.

### CSS Stylesheet (`src/styles/site.css`) Inspection
In `src/styles/site.css`, we observed:
* **`.pill--failed` Rule**: Defined on lines 325–330:
  ```css
  .pill--pending,
  .pill--unavailable,
  .pill--failed {
    background: rgba(245, 158, 11, 0.2);
    color: #fbbf24;
  }
  ```
  This renders failed items in yellow/orange.
* **Custom Variables inside `:root`**: Defined on lines 1–11:
  ```css
  :root {
    --site-bg: #060914;
    --site-panel: rgba(15, 23, 42, 0.82);
    ...
  }
  ```
* **Dead Prototype Files**:
  * `src/components/UnavailablePanel.astro` exists but is not imported or referenced anywhere in `src/`.
  * `docs/assets/factory-dashboard.css` and `docs/assets/factory-dashboard.js` are in the build directory `docs/assets/` and tracked in Git, violating standard Astro layout where static assets go to `public/`.
* **Unused CSS Rules**:
  * `.status-grid` (lines 127–131) is not used by any Astro elements.
  * `.status-card--muted` (lines 138–140) is not used.

### Testing and Build Configuration
We observed in `package.json`:
* **Build Script (Line 7)**:
  `"build": "rm -rf docs/.prerender docs/_astro docs/applications docs/tests docs/upstream docs/bluefin docs/adoption docs/homebrew && astro build"`
* **Test Script (Line 10)**:
  `"test": "node --test --test-concurrency=1 tests/astro-foundation.test.mjs tests/applications-page.test.mjs tests/adoption-page.test.mjs tests/homebrew-page.test.mjs"`
In `tests/astro-foundation.test.mjs`, we observed:
* Line 39 asserts that "Overview" navigation is NOT present:
  `assert.doesNotMatch(html('docs/index.html'), /site-nav__link[^>]*>Overview</, 'top nav no longer shows Overview tab');`

---

## 2. Logic Chain

1. **Skip Links**: Since there are no `.skip-link` styles or selectors in `site.css` and no skip link in `SiteLayout.astro`, keyboard users must tab through all navigation links on every page load. Adding an off-screen-till-focused link pointing to `<main id="main-content" tabindex="-1">` is needed to meet accessibility (WCAG 2.1) standards.
2. **SEO & Meta Tags**: Social crawlers rely on OG and Twitter tags to generate rich previews. Because these are missing in `SiteLayout.astro`, social shares default to standard body tags. Adding metadata props ensures dynamic, search-engine-friendly previews per page.
3. **Fonts**: While the body selector specifies `font-family: Inter...`, no link imports this font, causing browsers to fall back to generic system fonts. Connecting to Google Fonts or serving locally resolves this.
4. **Color-Scheme**: Enforcing `color-scheme: dark` at the browser layout level prevents flashes of light background during loading and ensures system scrollbars/input elements render in dark mode.
5. **Page Headers**: `bluefin.astro` lacks a page header containing `<h1>`. This causes it to fail standard document outlines. Introducing `<h1>Bluefin Upstream Status</h1>` aligns it with `upstream.astro`.
6. **Broken Links**: `castrojo/testing-lab` was renamed to `projectbluefin/lab`. Therefore, the methodology page footer links point to nonexistent locations. The `plan.md` file was also removed from the project root, so pointing to it produces a 404.
7. **Unused CSS/Dead Code**: A clean build system should minimize CSS footprint. Removing `.status-grid` and `.status-card--muted`, along with deleting `UnavailablePanel.astro` (which is never imported), cleans up dead elements.
8. **Build / Test Sync**: When adding `src/pages/about.astro`, the output will write to `docs/about/index.html`. If the `build` script in `package.json` does not clean this directory, outdated cached files could persist. Similarly, `tests/astro-foundation.test.mjs` must test the new about route output.

---

## 3. Caveats

* **Overview Tab Assertion**: The test `tests/astro-foundation.test.mjs` explicitly asserts that the Overview tab does NOT appear in the navigation bar. If we add "Overview" to the main nav menu, this test will fail and must be modified. If the site brand link (`.site-brand`) is used to highlight Overview, the test remains valid.
* **Local Fonts**: We assume fonts are loaded via Google Fonts CDN. If a completely offline local-first build is required, Inter `.woff2` font files should be placed in `public/fonts/` and registered via `@font-face` in `site.css`.

---

## 4. Conclusion

The codebase is highly functional but requires layout, SEO, navigation, and CSS sanitization adjustments to achieve production-grade quality. The following modifications are proposed:

### Proposed Layout (`src/layouts/SiteLayout.astro`) Improvements
1. **Add Skip Link**:
   * Insert at body start: `<a href="#main-content" class="skip-link">Skip to content</a>`
   * Update main wrapper: `<main id="main-content" class="site-shell" tabindex="-1">`
   * Define styles in `site.css`:
     ```css
     .skip-link {
       position: absolute;
       top: -9999px;
       left: -9999px;
       z-index: 100;
       background: var(--site-bg);
       color: var(--site-accent);
       padding: 1rem;
       border: 1px solid var(--site-border);
       border-radius: 8px;
     }
     .skip-link:focus {
       top: 10px;
       left: 10px;
       position: fixed;
     }
     ```
2. **Add Metadata Tags**:
   * Add Open Graph & Twitter meta tags mapping to `title` and `description` props.
3. **Add Favicon Link**:
   * Add `<link rel="icon" type="image/svg+xml" href={`${baseUrl}favicon.svg`} />` to `<head>`.
4. **Standardize Font and Color-Scheme**:
   * Preconnect to Google Fonts and load `Inter` font in `<head>`.
   * Add `<meta name="color-scheme" content="dark" />`.
5. **Standard Footer**:
   * Insert `<footer>` with copyright dynamic year, repo link, and build timestamp.
6. **Active Navigation Links**:
   * Map `navItems` in layout to support `bluefin` and `about` routes:
     ```javascript
     const navItems = [
       { id: 'bluefin', label: 'Bluefin', href: `${baseUrl}bluefin/` },
       { id: 'upstream', label: 'Upstream', href: `${baseUrl}upstream/` },
       { id: 'tests', label: 'Tests', href: `${baseUrl}tests/` },
       { id: 'applications', label: 'Applications', href: `${baseUrl}applications/` },
       { id: 'homebrew', label: 'Homebrew', href: `${baseUrl}homebrew/` },
       { id: 'adoption', label: 'Adoption', href: `${baseUrl}adoption/` },
       { id: 'userspace', label: 'Userspace', href: `${baseUrl}userspace/` },
       { id: 'about', label: 'About', href: `${baseUrl}about/` },
     ] as const;
     ```

### Proposed Bluefin Page (`src/pages/bluefin.astro`) Improvements
1. Add `<div class="dashboard-header">` with `<h1>Bluefin Upstream Status</h1>` (matching `upstream.astro`).
2. Update `current` property: change `current="upstream"` to `current="bluefin"`.

### Proposed Methodology Page (`src/pages/about.astro`) Conversion
1. Extract content from `docs/about/methodology.html`.
2. Convert it into Astro code importing `SiteLayout` and passing `current="about"`.
3. Wrap page tables with `.table-scroll` and `.data-table` to maintain UI consistency.
4. Replace footer links to refer to `https://github.com/projectbluefin/lab` and `https://github.com/projectbluefin/lab/tree/main/schemas/v2`. Replace `plan.md` with link to `AGENTS.md` or remove it.

### Proposed CSS Cleanups (`src/styles/site.css`)
1. Separate `.pill--failed` into its own rule block:
   ```css
   .pill--failed {
     background: rgba(251, 113, 133, 0.2);
     color: #fb7185;
   }
   ```
2. Define custom semantic root variables in `:root` (`--site-color-success`, `--site-color-warning`, `--site-color-danger`) and update the `.pill--` modifier rules to use them.
3. Delete the dead component `src/components/UnavailablePanel.astro`.
4. Delete the unused CSS rules `.status-grid` and `.status-card--muted`.

### Proposed Build & Test Changes
1. **`package.json`**: Update clean phase to remove `docs/about`:
   ```json
   "build": "rm -rf docs/.prerender docs/_astro docs/applications docs/tests docs/upstream docs/bluefin docs/adoption docs/homebrew docs/about && astro build"
   ```
2. **`tests/astro-foundation.test.mjs`**:
   * Add `'docs/about/index.html'` to `expectedFiles` array.
   * Add assertion: `assert.match(html('docs/about/index.html'), /Bluefin QA — Methodology/i);`
   * Update the Overview nav check if the Overview tab is re-added, or assert the active brand highlight instead.

---

## 5. Verification Method

### 1. Verification Commands
Run the following build and test commands from the root directory:
```bash
# Run the clean build to compile Astro files into the /docs directory
npm run build

# Run the Node test runner to execute the test suite
npm test
```

### 2. Files to Inspect
* Confirm compilation of `docs/about/index.html`.
* Open `docs/bluefin/index.html` and check that it contains exactly one `<h1>` tag: `<h1>Bluefin Upstream Status</h1>`.
* Verify that `docs/about/index.html` has correct GitHub links.
* Inspect `docs/_astro/SiteLayout.*.css` to verify `.pill--failed` color properties and presence of skip-link styles.

### 3. Invalidation Conditions
* The build output `docs/` is modified directly without updating the respective `.astro` file under `src/pages/`.
* The `npm test` suite fails due to route layout mismatches.
