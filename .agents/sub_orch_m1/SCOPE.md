# Scope: Milestone 1: Layout, SEO, Navigation, and CSS Cleaning

## Architecture
- Framework: Astro (static prerender mode).
- Page templates: Astro layouts (`src/layouts/SiteLayout.astro`) and pages (`src/pages/*.astro`, `src/pages/about.astro` migrated from `docs/about/methodology.html`).
- Styling: Custom global CSS rules in `src/styles/site.css`.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|--------------|--------|
| 1 | M1: Layout, SEO, Navigation, CSS Cleaning | Implement layout updates, add skip-link, footer, Open Graph, Inter font, adjust dark color-scheme, fix headings, migrate about page, fix fail pill color, introduce custom CSS semantic colors, clean unused rules/prototype files, run build and verify tests. | none | IN_PROGRESS |

## Interface Contracts
### Layout ↔ Pages
- `SiteLayout.astro` accepts standard props (e.g. `title`) and provides a single `<main>` content container for nested pages, utilizing a keyboard-focusable skip-to-content link pointing to a target container (e.g. `<main id="main-content">` or equivalent).
- Navigation in `SiteLayout.astro` correctly highlights the current page by checking `Astro.url.pathname`.
