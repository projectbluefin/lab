# Page-oriented dashboard data contracts

These contracts split the factory dashboard into page-owned JSON files so each deep page can load only the data it needs.

## Shared rules

All files follow the same starter pattern:

- `schema_version`: contract version for the file.
- `_meta`: artifact-level metadata (`page`, `description`, `generated_at`, `starter_artifact`, `status`).
- `summary_metrics[]`: page headline metrics. Every metric row must include `source_url`, `collected_at`, and `derivation`.
- `rows[]`: the primary page records. Every row must include:
  - `source_url`: canonical evidence link for the row.
  - `collected_at`: when this JSON row was assembled.
  - `derivation`: how the row was computed from source inputs.
  - `state`: `available` or `unavailable`.
  - `state_reason`: explicit reason when the row cannot support runtime claims yet.
- Placeholder values use `null` plus `state: "unavailable"`; collectors must never invent values.

## `docs/data/upstream-status.json`

Purpose: one row per tracked upstream stream for both `/upstream` (non-Bluefin families) and `/bluefin` (Bluefin, Bluefin-LTS, Dakota) pages.

### Top-level shape

- `_meta`
- `summary_metrics[]`
- `groups[]`: logical families shown in the page nav/filtering.
- `rows[]`: concrete upstream streams.

### Row shape

| Field | Meaning |
| --- | --- |
| `id` | Stable stream id (`bluefin-testing`, `fedora-bootc-stable`) |
| `group` | `gnome-os`, `fedora-bootc`, `projectbluefin`, `ublue` |
| `variant` | Product/stream name |
| `display_name` | Human label for the page |
| `publisher_repo` | Source repo when known |
| `org` | Owning org when known |
| `branch` | Stream/tag tracked by the collector |
| `published_at` | Upstream release publish time |
| `freshness_age_days` | Days since `published_at` |
| `open_prs` | Optional repo pressure signal |
| `state` / `state_reason` | Explicit availability contract |
| `source_url` / `collected_at` / `derivation` | Provenance for the row |

`published_at` for image lanes is sourced from the strongest available publish signal in this order:
1. GHCR package tag timestamps for lane tags (`stable`, `testing`) when available.
2. GitHub Releases `published_at` when package tag timestamps are unavailable.

## `docs/data/tests-matrix.json`

Purpose: one row per `(variant, branch, suite)` result for the `/tests` page.

### Top-level shape

- `_meta`
- `summary_metrics[]`
- `dimensions`: distinct variants/branches/suites for filters.
- `rows[]`: concrete matrix cells.

### Row shape

| Field | Meaning |
| --- | --- |
| `id` | Stable matrix key (`bluefin-testing-smoke`) |
| `variant` / `branch` / `suite` | Page filter dimensions |
| `result_status` | Published status from `docs/results/*.json` |
| `last_run` | Workflow completion time for the current cell |
| `workflow_name` | Workflow evidence for drill-down |
| `scenarios_total` / `scenarios_failed` | Current scenario counts |
| `pass_rate` | Derived percentage or `null` when unavailable |
| `history_points` | Count of historical entries already published |
| `results_path` / `screenshot_path` / `screenshot_url` | Artifact links |
| `state` / `state_reason` | Explicit availability contract |
| `source_url` / `collected_at` / `derivation` | Provenance for the row |

## `docs/data/applications-matrix.json`

Purpose: app-first rows for the `/applications` page. V1 currently tracks Bazaar and Firefox.

### Top-level shape

- `_meta`
- `summary_metrics[]`
- `applications[]`: app catalog entries.
- `rows[]`: one row per `(app_id, variant, branch)`.

### Application catalog shape

| Field | Meaning |
| --- | --- |
| `id` | Stable app id (`bazaar`, `firefox`) |
| `display_name` | Page label |
| `scope` | Current rollout scope (`v1`) |
| `primary_suite` | Preferred evidence source |
| `fallback_suites` | Coarser stop-gap evidence sources |
| `source_url` / `collected_at` / `derivation` | Provenance for the catalog entry |

### Row shape

| Field | Meaning |
| --- | --- |
| `id` | Stable key (`bazaar-bluefin-testing`, `firefox-bluefin-testing`) |
| `app_id` | Foreign key into `applications[]` |
| `variant` / `branch` | Page filter dimensions |
| `primary_suite` | Intended app evidence lane |
| `primary_result_status` | Published status for the primary suite |
| `primary_last_run` | Latest run for the primary suite |
| `scenario_total` / `scenario_failed` | App result totals when available |
| `fallback_signal_count` | Number of coarse fallback signals attached |
| `fallback_signals[]` | Optional coarse evidence rows (same provenance rules) |
| `state` / `state_reason` | Explicit availability contract |
| `source_url` / `collected_at` / `derivation` | Provenance for the row |

## Starter-artifact intent

These files are implementation-ready contracts plus honest seed data. Later collector work should replace starter `unavailable` rows with live evidence, not redesign the shape.

## `docs/data/homebrew-ecosystem.json`

Purpose: one row per tracked image lane, integrated into the `/adoption` page as supplementary ecosystem context. Covers Homebrew tap/package install and download statistics per `(variant, branch)`.

### Top-level shape

- `_meta`
- `summary_metrics[]`
- `taps[]`: Homebrew taps this repo explicitly tracks (empty until a repo-owned artifact fetched from formulae.brew.sh or upstream tap repos is added).
- `rows[]`: one row per `(variant, branch)` from `docs/data/variant-publishers.json`.

### Row shape

| Field | Meaning |
| --- | --- |
| `id` | Stable lane id (`bluefin-testing`, `aurora-stable`) |
| `variant` | Image variant name |
| `branch` | Stream/tag (`testing`, `stable`) |
| `tap_name` | Homebrew tap name when known, else `null` |
| `tap_url` | Canonical tap URL when known, else `null` |
| `install_count` | Total installs from brew stats artifact, or `null` when unavailable |
| `download_count` | Total downloads from brew stats artifact, or `null` when unavailable |
| `state` / `state_reason` | Explicit availability contract |
| `source_url` / `collected_at` / `derivation` | Provenance for the row |

### Summary metrics

| id | Meaning |
| --- | --- |
| `tracked_image_lanes` | Total lanes from `variant-publishers.json` |
| `lanes_with_brew_data` | Lanes with Homebrew analytics data from formulae.brew.sh or upstream tap repos present in docs/data/ |
| `lanes_awaiting_brew_data` | Lanes with no Homebrew analytics data from formulae.brew.sh or upstream tap repos in docs/data/ |

## `docs/data/adoption-metrics.json`

Purpose: executive-readable adoption view for the `/adoption` page. Covers image pull counts from container registry APIs (GHCR), active-device estimates from Fedora countme infrastructure, and trust/provenance coverage per tracked image lane.

### Top-level shape

- `_meta`
- `summary_metrics[]`
- `trust_cards[]`: one card per tracked variant with static trust/provenance metadata.
- `rows[]`: one row per `(variant, branch)` with pull and countme signals from authoritative upstream sources.

### Trust card shape

| Field | Meaning |
| --- | --- |
| `variant` | Image variant name |
| `publisher_repo` | Source repo when known |
| `org` | Owning org |
| `emits_sbom` | Whether the publisher emits an SBOM |
| `emits_cve_scan` | Whether the publisher emits a CVE scan |
| `emits_cosign_attestation` | Whether the publisher emits a cosign attestation |
| `state` / `state_reason` | `available` when `publisher_repo` and `org` are known; `unavailable` with explicit `state_reason` when the publisher is unknown (e.g., flatcar) |
| `source_url` / `collected_at` / `derivation` | Provenance for the card |

### Row shape

| Field | Meaning |
| --- | --- |
| `id` | Stable lane id (`bluefin-testing`, `bluefin-lts-stable`) |
| `variant` | Image variant name |
| `branch` | Stream/tag |
| `pull_count` | Registry pull count from container registry API (e.g., GHCR package statistics), or `null` when unavailable |
| `countme_active_devices` | Active device estimate from Fedora countme infrastructure, or `null` when unavailable |
| `state` / `state_reason` | Explicit availability contract |
| `source_url` / `collected_at` / `derivation` | Provenance for the row |

### Summary metrics

| id | Meaning |
| --- | --- |
| `tracked_image_lanes` | Total lanes from `variant-publishers.json` |
| `lanes_with_pull_data` | Lanes with pull_count from container registry API (e.g., GHCR) present in docs/data/ |
| `lanes_with_countme_data` | Lanes with countme_active_devices from Fedora countme infrastructure present in docs/data/ |

## release-verdict.json (index / SRE triage)

Written by `scripts/collect_release_verdict.py` (ADR 0002). One row per production lane
(`bluefin-stable`, `bluefin-testing`, `bluefin-lts-stable`, `bluefin-lts-testing`, `dakota-testing`).

### Top-level shape

- `_meta`: `generated_at`, `source`, `verdict_definition` (pointer to ADR 0002)
- `lanes[]`: one verdict row per lane

### Lane row shape

| Field | Meaning |
| --- | --- |
| `lane` | Stable lane id |
| `image` / `tag` | GHCR image reference |
| `digest` | Current manifest digest resolved anonymously from GHCR, or `null` when unavailable |
| `verdict` | `good` iff build passed AND lab QA passed on this digest AND cosign keyless verify passed; `bad` when any input failed; `pending` when an input has no evidence for this digest |
| `inputs.build` / `inputs.qa` / `inputs.signature` | Each `{status: passed\|failed\|pending\|unavailable, detail, source_url}` |
| `state` / `state_reason` | Explicit availability contract |
| `source_url` / `collected_at` / `derivation` | Provenance for the row |

Notes:
- Signature verification is keyless (`--certificate-identity-regexp '^https://github.com/projectbluefin/'`,
  GitHub Actions OIDC issuer). Verify results are cached by digest in the previous JSON.
- QA evidence must reference the current digest's build or newer; stale evidence yields `pending`, never `good`.
- CVE counts are displayed alongside but never gate the verdict (ADR 0002).

## history/release-verdict.ndjson

Rolling append-only history of verdict transitions. One line per `(lane, digest)` change:
`{recorded_at, lane, digest, verdict, inputs_summary}`. Retention: 365 days; the collector
prunes older lines on each run. Rows never rewrite — a new digest or changed verdict appends.
