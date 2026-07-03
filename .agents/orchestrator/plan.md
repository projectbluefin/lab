# Implementation Plan - factory.projectbluefin.io dashboard improvements

This plan lays out the detailed milestones for refactoring the dashboard from a hybrid client-side rendered codebase into a build-time prerendered modern Astro app, with component extraction, style cleaning, pipeline hardening, and type safety checks.

## Milestones

### Milestone 1: Layout, SEO, Navigation, and CSS Cleaning
- **Target Files**:
  - `src/layouts/SiteLayout.astro`
  - `src/styles/site.css`
  - `docs/about/methodology.html` -> `src/pages/about.astro`
- **Tasks**:
  - Ensure every page (including `/bluefin/`) has exactly one `<h1>` heading.
  - In `SiteLayout.astro`, insert a keyboard-focusable skip-to-content link (skipping navigation to the main content area).
  - Add standard footer to `SiteLayout.astro` with copyright, project repo link, and build timestamp.
  - Update broken/placeholder GitHub links in the footer.
  - Adjust color-scheme to `dark` via meta tags, add favicon link, load Inter font via Google Fonts, and add Open Graph/Twitter meta tags.
  - Update nav in `SiteLayout.astro` to add "Overview" pointing to `/`. Make sure `/bluefin/` and `/about/` highlight correctly.
  - Convert `docs/about/methodology.html` to `src/pages/about.astro` using `SiteLayout.astro`.
  - Fix `.pill--failed` to color `#fb7185` (red) instead of amber in `site.css`.
  - Define custom semantic status variables (`--status-passed`, `--status-failed`, etc.) in `site.css`.
  - Clean up dead prototype files and unused CSS rules.
- **Verification**:
  - Astro page builds cleanly (`npm run build`).
  - Unit tests run with `npm test`.

### Milestone 2: Component Extraction and Upstream Dedup
- **Target Files**:
  - `src/components/MetricCard.astro`
  - `src/components/DetailCard.astro`
  - `src/components/EvidenceLinks.astro`
  - `src/components/DataIntegrityBlock.astro`
  - `src/components/ChartWrapper.astro`
  - `src/components/MatrixTable.astro`
  - `src/components/SectionHeading.astro`
  - `src/pages/upstream.astro`, `src/pages/bluefin.astro`
  - `src/scripts/chart-utils.js` (new)
- **Tasks**:
  - Extract reusable components to eliminate duplicate HTML markup.
  - Consolidate `upstream.astro` and `bluefin.astro` to avoid style/HTML duplication, using parameterized props or a shared sub-layout.
  - Move inline chart script tags to external JS modules. Create `src/scripts/chart-utils.js` containing ECharts setup, resize debouncing (150ms), and empty-state handling.
- **Verification**:
  - Pages render correctly post-extraction.
  - `npm run build` succeeds.

### Milestone 3: Homepage Migration and Asset Cleanup
- **Target Files**:
  - `src/pages/index.astro`
  - `docs/assets/factory-dashboard.js` (delete)
  - `docs/assets/factory-dashboard.css` (delete)
- **Tasks**:
  - Rewrite `src/pages/index.astro` to load and parse `factory-stats.json` and `factory-telemetry.json` at build time.
  - Replace the old client-side mounting container `#factory-dashboard` with structured cards and tables.
  - Remove legacy client-side scripts/styles from `docs/assets/` and disable `includeDashboardAssets` logic.
- **Verification**:
  - `npm test` checks for the updated index landing page structure.

### Milestone 4: Pipeline Hardening and Testing
- **Target Files**:
  - `.github/workflows/update-test-results.yml`
  - `scripts/refresh_factory_stats.py` (new)
  - `tests/...`
- **Tasks**:
  - Move ~500 lines of inline Python in `update-test-results.yml` to `scripts/refresh_factory_stats.py`.
  - Add concurrency groups to `update-test-results.yml` to prevent concurrent git commit race conditions.
  - Add JSON contract validation tests to the test suite for `tests-matrix.json`, `upstream-status.json`, `applications-matrix.json`, and `factory-stats.json`.
  - Add a rendering test for `/userspace/`.
- **Verification**:
  - `npm test` runs green.

### Milestone 5: Type Safety
- **Target Files**:
  - `src/lib/` files
  - Astro page frontmatters
- **Tasks**:
  - Export clean TS interfaces.
  - Type all Astro pages' frontmatter data.
  - Replace implicit `any` template casts with typed model interfaces.
- **Verification**:
  - `npm run check` completes with zero errors.

## Overall Test Execution
For all milestones:
- Run `npm test` and `npm run check` via worker agents.
- Perform visual/layout validation of built HTML pages.
