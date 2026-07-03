# Handoff Report - Factory Dashboard Codebase Investigation

## 1. Observation
We investigated the repository at `/var/home/jorge/src/testing-lab` and observed the following components of the `factory.projectbluefin.io` dashboard:

### Astro Config and Site Structure (`src/`)
- **Astro Config**: `/var/home/jorge/src/testing-lab/astro.config.mjs`
  - Specifies output strategy and destination:
    ```javascript
    export default defineConfig({
      output: 'static',
      outDir: './docs',
      site: 'https://factory.projectbluefin.io',
      trailingSlash: 'always',
      // ...
    });
    ```
- **Base Layout**: `src/layouts/SiteLayout.astro`
  - Renders base HTML skeleton and navigation links (`upstream`, `tests`, `applications`, `homebrew`, `adoption`, `userspace`).
  - Conditionally includes legacy CSS and JS dashboard assets when `includeDashboardAssets` prop is `true`:
    ```astro
    {includeDashboardAssets && (
      <>
        <link rel="stylesheet" href={`${baseUrl}assets/factory-dashboard.css`} />
        <script is:inline src={`${baseUrl}assets/factory-dashboard.js`} defer data-cfasync="false"></script>
      </>
    )}
    ```
- **Pages**:
  - `src/pages/index.astro`: Mounts the `#factory-dashboard` container element and requests the legacy client-side assets by setting `includeDashboardAssets={true}`.
  - `src/pages/upstream.astro`: Processes `docs/data/upstream-status.json` at build time (using `buildUpstreamPageModel` from `src/lib/upstream-page.js`), excludes `projectbluefin` variants, and renders ECharts containers.
  - `src/pages/bluefin.astro`: Same as `upstream.astro` but filters to include only `projectbluefin` variants.
  - `src/pages/tests.astro`: Renders VM test surface matrices and coverage statistics.
  - `src/pages/applications.astro`: Hardcodes the metadata for Firefox, Ptyxis, Codium, and Podman Desktop inside `bluefinDefaultApps` and displays test outcome grids.
  - `src/pages/adoption.astro`: Displays user-adoption countme numbers and active device trends.
  - `src/pages/homebrew.astro`: Renders tap packages statistics and formulae.brew.sh leaderboard.
  - `src/pages/userspace.astro`: Lists FSDK container build progress and local Zot registry cache stats.
- **Client-Side Scripts & Styles**:
  - `src/scripts/upstream-page.js`: Client-side module configuring/initializing ECharts instances (`upstream-availability-chart`, `upstream-freshness-chart`, `upstream-timeline-chart`).
  - `src/scripts/tests-charts.js`: Configures ECharts for the test matrix page.
  - `src/styles/site.css`: Main styling stylesheet for the modern Astro shell.

### Static Methodology Resource
- **Location**: `docs/about/methodology.html`
  - A standalone, styled static HTML page documenting derived metric estimator formulas (e.g., Wilson score interval) and valid snapshot state vocabularies (e.g., `partial`, `degraded`, `stale`).

### JSON Data Files
- **Locations (all inside `docs/data/`)**:
  - `docs/data/factory-stats.json`: Holds live cluster node stats, recent workflow runs, image release tags, and test coverage aggregations.
  - `docs/data/factory-telemetry.json`: Public SLSA/DORA lineage data including hash digests of result files, Wilson confidence, and queue pressure.
  - `docs/data/tests-matrix.json`: Build-time test outcome matrix data.
  - `docs/data/upstream-status.json`: Transformed upstream lane statuses.
  - `docs/data/applications-matrix.json`: Build-time application metrics.

### Legacy Scripts/CSS to Delete
- **Locations**:
  - `docs/assets/factory-dashboard.js`: Contains 1,059 lines of vanilla client-side JavaScript that performs AJAX fetches for stats/telemetry JSON files and manually inserts HTML into the overview dashboard shell.
  - `docs/assets/factory-dashboard.css`: Stylesheet used by the legacy client-side dashboard shell.

### Workflow Files
- **Location**: `.github/workflows/update-test-results.yml`
  - Triggered on a cron schedule (`*/5 * * * *`) and on push to main for tracked paths.
  - Performs ORAS pulls for screenshots, runs a Python script block to refresh `factory-stats.json` (fetching live Argo/Node logs if on a homelab runner), runs `python3 scripts/generate_page_datasets.py` to regenerate JSON files, runs `npm run build` and `npm test`, and commits the updated `docs/` artifacts.

### Test Suites
- **Location**: `tests/` directory.
- Test runner invocation: `npm test` (mapped to `node --test` in `package.json`).
- Core test files:
  - `tests/astro-foundation.test.mjs`: Tests page builds and verifies structure of output files under `docs/`.
  - `tests/applications-page.test.mjs`: Validates the structure and elements of `docs/applications/index.html`.
  - `tests/adoption-page.test.mjs`: Verifies `docs/adoption/index.html` structure and validates the JSON schema schema-compliance of `docs/data/adoption-metrics.json`.
  - `tests/homebrew-page.test.mjs`: Asserts layout properties and contract validation of `docs/data/homebrew-ecosystem.json`.

---

## 2. Logic Chain
1. **Legacy Dependency Elimination**: 
   - `src/pages/index.astro` is the only route that mounts the container `#factory-dashboard` and queries the legacy scripts by setting `includeDashboardAssets={true}` (from `src/layouts/SiteLayout.astro`).
   - If we rewrite the index page (`src/pages/index.astro`) to render its telemetry metrics and recent runs lists at build time (similar to `src/pages/upstream.astro` and others) utilizing server-side Astro templates, we can safely toggle `includeDashboardAssets` to `false` and delete `docs/assets/factory-dashboard.js` and `docs/assets/factory-dashboard.css`.
2. **ECharts Performance & Safety**:
   - `src/pages/upstream.astro` serializes its data using a custom `<script type="application/json">` block, which is then parsed by `src/scripts/upstream-page.js` on the client side to avoid inline JS. This matches the project security standards (Cloudflare and sandbox safety).
3. **Pipeline & Test Guardrails**:
   - Running `npm test` invokes the full node verification suite. Because these tests perform a full `npm run build` and inspect the generated HTML files inside `docs/`, any modification to dashboard files or removal of legacy scripts must be accompanied by updates to the test assertions in `tests/astro-foundation.test.mjs`.

---

## 3. Caveats
- **Argo NodePort Connection**: Part of the `update-test-results.yml` workflow queries `https://192.168.1.102:32746/api/v1/workflows/argo`. When run outside the ghost node or local homelab networks (e.g. standard GitHub-hosted runners), this connection will timeout and fall back to the last known committed telemetry, marking the status as `degraded` or `unknown`. This is normal.
- **No Implementation Allowed**: Per agent constraints, we did not modify any source code or delete the legacy files.

---

## 4. Conclusion
The dashboard operates as a hybrid build-time Astro application. The main dashboard index is client-side rendered using legacy JS (`factory-dashboard.js`), whereas the subpages are statically generated at build time using JSON datasets inside `docs/data/`.
The codebase is clean, tests are passing, and the path to implementation is straightforward:
1. Re-implement `src/pages/index.astro` in native Astro components using build-time data sources (`docs/data/factory-stats.json` and others).
2. Clean up `includeDashboardAssets` logic in `src/layouts/SiteLayout.astro`.
3. Delete `docs/assets/factory-dashboard.js` and `docs/assets/factory-dashboard.css`.
4. Update the node test suite to match the restructured landing page.

---

## 5. Verification Method
- **Local Test Execution**:
  Run the test suite from the repository root:
  ```bash
  npm test
  ```
  Expected output: `pass 13`, `fail 0`.
- **Astro Build Execution**:
  Validate static page generation:
  ```bash
  npm run build
  ```
  Check that the output folder `./docs` is correctly populated with `index.html` and subdirectories.
