# Tests page data integrity and real-chart fix

Date: 2026-07-03
Status: approved
Scope: dashboard-side data integrity fix (Scope A). Live Argo-to-`docs/results/` ingestion
pipeline (Scope B) is explicitly out of scope for this pass and is called out as a follow-up
below.

## Problem

`/tests/` (`src/pages/tests.astro`) renders a variant x suite matrix and four Apache ECharts
panels (reliability trend, failure concentration, suite/variant heatmap, scenario volume) from
`docs/data/tests-matrix.json`, which is generated from `docs/data/test-surface.json` and
`docs/results/*.json` by `scripts/generate_page_datasets.py::build_tests_matrix`.

`docs/data/test-surface.json` is hand-authored and has drifted from reality:

- It lists `aurora` and `bazzite` as tested variants. Neither has a QA pipeline in this repo.
  They were borrowed from `docs/data/variant-publishers.json`, which is a broader
  ecosystem-comparison list (used by `/images/`) unrelated to what this repo's Argo pipelines
  actually test. All 8 rows for these two variants are `state: unavailable` placeholders that
  will never fill in — they are dead weight, not "pending" real coverage.
- The real, currently-tested surface is exactly:
  - **Variants**: `bluefin`, `bluefin-lts`, `dakota`, `flatcar` — confirmed by reading
    `argo/workflow-templates/bluefin-qa-pipeline.yaml`, `dakota-qa-pipeline.yaml`, and
    `run-flatcar-tests.yaml`, which are the only pipelines that provision a VM and run the
    `projectbluefin/testsuite` behave/qecore suite via `run-gnome-tests.yaml` /
    `run-flatcar-tests.yaml`.
  - **Suites**: `smoke`, `common`, `developer`, `software`, `system` — the default suite list in
    `bluefin-qa-pipeline.yaml`/`dakota-qa-pipeline.yaml`. `system` is already correctly aliased
    to testsuite's renamed `tests/lifecycle/` directory inside `run-gnome-tests.yaml:298-301` —
    this mapping is correct today and must be preserved, not "fixed."
- `docs/results/*.json` for the 4 real variants already contain genuine historical run data
  (real workflow run names, timestamps, scenario/failure counts) — this was seeded once
  (~2026-06-24/25) and is not continuously refreshed by CI (no `argo/workflow-templates/*.yaml`
  writes into `docs/results/`), but it is real historical evidence, not fabricated.
- The honesty mechanism in `row_state()` (`state: unavailable` + a human-readable reason when
  `last_run` is null) already works correctly. The bug is the row *list* including variants that
  should never appear, not the state logic.
- The four ECharts panels in `TestsCharts.astro` / `src/scripts/tests-charts.js` are already
  well-designed (trend, failure concentration, heatmap, volume) and consume the matrix payload
  generically. They do not need new chart types — removing the 8 dead rows and correcting the
  underlying data automatically fixes what they render.

Out of scope (explicit boundary, confirmed with user): Knuckle (`knuckle-qa-pipeline.yaml`) and
`bluefin-server-build-pipeline.yaml` are architecturally different pipelines (installer smoke
tests / build-and-boot-only, not the `projectbluefin/testsuite` behave matrix) and are not folded
into this page.

## Changes

1. **`scripts/generate_page_datasets.py`**
   - Replace the hand-authored variant/suite enumeration that feeds `test-surface.json` with a
     derivation from the real, pipeline-confirmed set: variants
     `bluefin, bluefin-lts, dakota, flatcar`; suites
     `smoke, common, developer, software, system`.
   - Add a live cross-check against `projectbluefin/testsuite`'s GitHub tree using the same
     pattern already established in `src/pages/applications.astro:130-160`: `curl -fs -H
     "User-Agent: lab-builder" --max-time 3
     https://api.github.com/repos/projectbluefin/testsuite/git/trees/main?recursive=1`, parse for
     `.feature` files, and fall back to a static snapshot list on any failure (rate limit,
     network unavailable at build time). This cross-check is a warning/log-only safety net (does
     not fail the build) so a future testsuite rename or category addition is visible in build
     logs instead of silently drifting unnoticed, the way `aurora`/`bazzite` did.
   - `row_state()` and the general matrix-building logic are unchanged.
2. **Regenerate data**: run the generator to rebuild `docs/data/test-surface.json` (22 rows -> 20
   rows) and `docs/data/tests-matrix.json`. Delete the now-orphaned result files:
   `docs/results/bluefin-aurora-testing-{developer,smoke,software,system}.json` and
   `docs/results/bluefin-bazzite-testing-{developer,smoke,software,system}.json` (8 files total,
   all placeholders, verified above to carry zero real data).
3. **No changes** to `src/components/TestsCharts.astro` or `src/scripts/tests-charts.js` — they
   already consume the matrix payload generically.
4. **Tests**: update `tests/unit/test_page_dataset_collector.py` (and any other test asserting
   row counts, `aurora` presence, or the 22-row total) to reflect the corrected 20-row surface and
   the new variant/suite derivation.
5. **Verification**: `npm run build` (regenerates `docs/tests/index.html`), full test suite
   (`npm test` + relevant `pytest` targets), commit, push (rebasing against the automated
   `chore(derive)` bot commits as needed per established session convention), verify live on
   `factory.projectbluefin.io/tests/`.

## Follow-up (not in this pass)

Scope B — wiring `argo/workflow-templates/run-gnome-tests.yaml` to convert real behave JSON
output (it already writes `--format json.pretty` results to
`/var/mnt/ghost-data/test-results/{{workflow.name}}/${SUITE}`) into the dashboard's
`docs/results/` schema and publish it back to this repo after every real test run, so the page is
continuously live instead of periodically reseeded. This is a larger, cross-cutting change
(Argo workflow authoring + git push-back mechanism) and is deferred to a separate design/plan.
