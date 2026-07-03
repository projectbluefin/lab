# Project: factory.projectbluefin.io dashboard improvements

## Architecture
- Static site generator: Astro (static prerender mode outputting to `./docs`).
- Data flow: Build-time JSON data files located under `docs/data/` (or `public/data/` copied to `docs/data/`) are read by Astro page frontmatters and rendered into HTML.
- Layouts: `SiteLayout.astro` is the central layout shell.
- Subpages: Statically compiled at build time.
- External charts: ECharts rendered client-side on containers initialized by external JS modules.

## Code Layout
- `src/layouts/SiteLayout.astro` - Main layout shell.
- `src/pages/` - Static pages.
  - `index.astro` - Homepage (Overview dashboard).
  - `upstream.astro` - Upstream metrics (excluding projectbluefin).
  - `bluefin.astro` - Upstream metrics (including only projectbluefin).
  - `tests.astro` - Build-time test outcome matrix.
  - `applications.astro` - Default applications test outcomes.
  - `about.astro` - Methodology documentation (migrated from html).
  - `userspace.astro` - Container builds and Zot registry cache.
- `src/scripts/` - Client-side scripts for ECharts.
- `src/styles/site.css` - Custom styling stylesheet.
- `.github/workflows/update-test-results.yml` - Test results refresh pipeline.
- `scripts/` - Standalone python scripts for pipeline.
- `tests/` - Node.js testing library tests.

## Milestones
| # | Name | Scope | Dependencies | Status | Agent Conv ID |
|---|---|---|---|---|---|
| 1 | M1: Layout, SEO, Nav, CSS | Update SiteLayout.astro, site.css, migrate methodology to src/pages/about.astro, fix styling/headings. | None | IN_PROGRESS | 0fa17b3c-36f3-4b98-966d-d0034bfaa770 |
| 2 | M2: Astro Components | Extract MetricCard, DetailCard, EvidenceLinks, DataIntegrityBlock, ChartWrapper, MatrixTable, SectionHeading. Dedup upstream.astro & bluefin.astro. Extract chart-utils.js. | M1 | PLANNED | - |
| 3 | M3: Prerender Homepage | Rewrite index.astro to build-time prerender stats & telemetry JSON, remove legacy JS/CSS assets. | M2 | PLANNED | - |
| 4 | M4: Pipeline & Tests | Move inline Python to refresh_factory_stats.py, add concurrency groups, JSON contract tests, /userspace/ page test. | M3 | PLANNED | - |
| 5 | M5: TypeScript Typing | Export TypeScript interfaces, type all pages frontmatter. | M4 | PLANNED | - |

## Interface Contracts
- **Data Models**:
  - `factory-stats.json`
  - `factory-telemetry.json`
  - `tests-matrix.json`
  - `upstream-status.json`
  - `applications-matrix.json`
- **TypeScript Types**:
  - Interfaces exported from `src/lib/` files representing dashboard statistics, telemetry data, and test matrix contracts.
