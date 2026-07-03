# Original User Request

## 2026-07-01T21:33:23Z

Improve the factory.projectbluefin.io static dashboard based on the approved audit plan: migrate the homepage to build-time Astro prerendering, clean up inline script/style duplication, fix the layout/SEO heading hierarchy, extract reusable components, and harden the Python-YAML update pipeline.

Working directory: /var/home/jorge/src/testing-lab
Integrity mode: benchmark

## Requirements

### R1. Layout, SEO, and CSS Cleaning (Phases 1 & 5)
- **Layout & SEO**: Update `SiteLayout.astro` to render a single `<h1>` page heading. Add a skip-to-content keyboard link, a standard footer (with copyright, repo link, and build timestamp), a favicon link, and Open Graph/Twitter meta tags. Ensure Inter font is loaded via Google Fonts. Adjust the color-scheme meta tag to `dark`.
- **Navigation & Page Fixes**: Add "Overview" (pointing to `/`) to the nav. Ensure `/bluefin/` highlights correctly and has an `<h1>`. Migrate the legacy `/about/methodology.html` to an Astro page `/about/` using the shared layout, and update the three broken GitHub links in the footer.
- **Visuals**: Fix `.pill--failed` to color red (`#fb7185`) instead of amber. Add custom semantic color variables (`--status-passed`, `--status-failed`, etc.) to `:root` in `site.css`. Remove dead prototype files and unused CSS rules.

### R2. Astro Component Extraction (Phase 2)
- **Extract UI components**: Create reusable Astro components for `MetricCard`, `DetailCard`, `EvidenceLinks`, `DataIntegrityBlock`, `ChartWrapper`, `MatrixTable`, and `SectionHeading` to eliminate duplicated markup.
- **Upstream Dedup**: Consolidate `upstream.astro` and `bluefin.astro` template duplication by routing through a parameterized view or shared layout.
- **Chart Utilities**: Extract inline script tags on applications, adoption, homebrew, and userspace pages to external JS modules. Create `src/scripts/chart-utils.js` to share ECharts setup, resize debouncing (150ms), and empty-state rendering. Reference CSS variables for ECharts colors.

### R3. Homepage Migration (Phase 3)
- **Migrate Overview**: Rewrite `src/pages/index.astro` to read `factory-stats.json` and `factory-telemetry.json` at build-time (prerendered). Replace the noscript shell with fully structured cards and tables.
- **Remove legacy assets**: Delete `docs/assets/factory-dashboard.js` and `docs/assets/factory-dashboard.css`.

### R4. Pipeline Hardening and Testing (Phase 4)
- **YAML Hardening**: Move the ~500 lines of inline Python in `.github/workflows/update-test-results.yml` to a standalone script `scripts/refresh_factory_stats.py` and run it via `python3`.
- **Pipeline Controls**: Add concurrency groups to the update workflow to prevent race conditions during automatic git commits.
- **Tests**: Add JSON contract tests for `tests-matrix.json`, `upstream-status.json`, `applications-matrix.json`, and `factory-stats.json`. Add a page rendering test for `/userspace/`. Ensure `npm test` runs cleanly.

### R5. Type Safety (Phase 6)
- **TypeScript**: Export clean interfaces from `src/lib/` files. Type all Astro page frontmatters and replace implicit `any` template casts with typed model interfaces. Ensure `npm run check` completes.

## Acceptance Criteria

### CSS and Layout
- [ ] Every page has exactly one `<h1>` element.
- [ ] The skip-nav link is present and keyboard-focusable.
- [ ] Nav pills highlight correctly, including `/bluefin/` and `/about/`.
- [ ] `.pill--failed` displays red color.
- [ ] Legacy JS dashboard assets are deleted and the homepage is fully prerendered.

### Component Reuse
- [ ] Duplicate markup for KPI cards, detail cards, and data integrity sections is removed in favor of extracted components.
- [ ] No inline script tags exist for initializing charts on the page.
- [ ] `upstream.astro` and `bluefin.astro` have zero duplicated styling/HTML.

### Pipeline and Tests
- [ ] Inline Python is removed from `.github/workflows/update-test-results.yml`.
- [ ] A concurrency lock is configured in the update workflow.
- [ ] `npm test` runs successfully, validating all page contracts and rendering tests (including `/userspace/`).
