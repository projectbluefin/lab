---
name: ci-tooling
description: >
  GitHub Actions workflow authoring and debugging for lab dashboards and
  automation. Use when changing .github/workflows/, troubleshooting stale Pages
  data, or wiring CI jobs that need private-cluster data.
metadata:
  context7-sources:
    - /websites/github_en_actions
    - /websites/github_en_rest
---

# CI Tooling — GitHub Actions in lab

## When to Use

- Editing `.github/workflows/*.yml`
- Dashboard data is stale, empty, or inconsistent with cluster state
- A workflow needs homelab/private network data
- GitHub Pages shows JSON/JS fetch errors after CI changes

## When NOT to Use

- Argo WorkflowTemplate logic in `argo/workflow-templates/` (use `argo-workflows.md`)
- ArgoCD reconciliation policy work (use `gitops-argocd.md`)
- VM lifecycle/scheduling behavior (use `kubevirt-vms.md`)

## Core Process

1. Confirm runner network model first: GitHub-hosted runners have public internet by default; private network access requires an overlay/VPN setup or a self-hosted runner.
2. For dashboard stats jobs, treat private-cluster snapshots as optional: when live fetch fails, preserve last known live values and set explicit freshness/state flags.
3. Never wipe `recent_runs` or `factory.cluster.nodes` just because a hosted runner cannot reach `192.168.x.x`; preserve and annotate.
4. Add explicit metadata in JSON (`_meta.live_snapshot_ok`, `_meta.refreshed_at`) so UI can show freshness honestly.
5. For GitHub Pages sites, verify the Pages source and build state before assuming a push is live:
   - `gh api repos/<owner>/<repo>/pages --jq '.source'`
   - `gh api repos/<owner>/<repo>/pages/builds/latest --jq '.status'`
   - Pages can legitimately stay in `building` for a while even after the commit is on `main`.
6. If Cloudflare fronts a Pages site, inspect the live HTML for Rocket Loader rewrites (`data-cf-settings`, `type="...-text/javascript"`). Opt the dashboard entry script out with `data-cfasync="false"` when RL rewrites break execution.
7. For dashboard data jobs, confirm the producer workflow published every generated JSON artifact the UI depends on before declaring a section broken; missing data should render an explicit unavailable state rather than disappearing.
8. For browser-side `fetch`, avoid custom request headers that force CORS preflight against GitHub APIs (for example `Cache-Control` request headers).
9. After push, validate production Pages with a real browser render (not raw HTML fetch only): confirm no loading placeholders and key sections render.
10. Any image freshness/status numbers shown in the dashboard must come from source-of-truth publish metadata: prefer GHCR package tag timestamps (`/orgs/{org}/packages/container/{name}/versions`) for `stable`/`testing` lane recency, then fallback to GitHub Releases metadata (`published_at`, `tag_name`, `html_url`) when package tags are not available.
11. Do not infer image freshness from Argo workflow names, run labels, or generic CI success events. Those are execution signals, not publish timestamps.
12. For every dashboard number that is not self-evident from local files, publish explicit source lineage (source URL + derivation input) or hide the number as unavailable.
13. For page-owned dashboard JSON (`docs/data/*-status.json`, `*-matrix.json`), keep row-oriented contracts stable: every summary metric and every row gets `source_url`, `collected_at`, `derivation`, plus explicit `state`/`state_reason` fields so later collectors can fill values without redesigning the shape.
14. Extract large inline scripts (especially Python/bash blocks over ~10-15 lines) from GHA YAML workflow files into standalone executable scripts under `scripts/`. This enables independent local execution, linting, testing, and modular maintenance.
15. Configure explicit GHA concurrency limits (`concurrency:`) on any automated workflow that commits/pushes files back to git. Use a unique group name (e.g. `group: update-test-results`) and set `cancel-in-progress: true` to prevent race conditions and rebase conflicts when multiple runs trigger in rapid succession.
16. After changing collector logic (`scripts/refresh_factory_stats.py` or `scripts/generate_page_datasets.py`), run the same local refresh sequence as CI (`refresh_factory_stats.py` → `generate_page_datasets.py` → `npm run build`) before handoff.
17. **Network timeouts are mandatory for any cluster-facing call in CI**: every `execSync`, `fetch`, `curl`, `skopeo`, or similar command that reaches `192.168.1.x` or any private endpoint must have an explicit timeout (e.g. `timeout: 2000` for Node.js, `--max-time` for curl, or socket timeout). Without timeouts, a single unreachable endpoint will hang the entire GitHub-hosted runner indefinitely, starving the concurrency group and blocking all downstream runs.
18. **Build-time assertions must handle environment drift**: if a test asserts presence of specific live data (e.g. `:30501` registry port), but the build environment differs from the test environment (GitHub runners → no homelab network access), the assertion will always fail. Instead: allow the code to handle missing data gracefully (e.g. fallback rendering), and update the assertion to accept both the live path and the fallback path. This prevents false CI failures that don't reflect real code bugs.
19. **Prefer the Kubernetes API server's service/pod proxy subresource over new NodePorts/manifests** when a collector needs to reach a ClusterIP-only service or a pod's non-Service-exposed diagnostics port: `kubectl get --raw "/api/v1/namespaces/<ns>/services/<svc>:<port>/proxy/<path>"` or `.../pods/<pod>:<port>/proxy/<path>`. This routes through the API server the runner already reaches (the same reachability `kubectl get nodes` relies on), so no cluster manifest changes are needed to expose a new metrics/status endpoint.
20. When a data source has no direct "value used" gauge (e.g. Buildbarn's block-device-backed storage, which preallocates fixed-size blocks on disk regardless of logical fill), derive an estimate from available counters (e.g. `allocations_total - releases_total` × known block size) and say so explicitly in the row's `derivation` field. Never silently report a physical-allocation number as if it were logical usage.

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "If live fetch fails, clearing fields is safer." | Clearing makes the dashboard lie by omission; preserve last known and mark freshness/state. |
| "Custom `Cache-Control` headers are harmless." | They can trigger CORS preflight and block cross-origin GitHub API fetches. |
| "Raw JSON looks right, so UI is fine." | JS/runtime errors can still break rendering; always validate in a browser. |

## Red Flags

- Workflow writes empty arrays for cluster/runs after transient network failure
- Dashboard shows `Loading…` or stale placeholder rows for long periods
- Live HTML shows Rocket Loader rewriting the dashboard entry script
- Generated dashboard sections disappear because their JSON artifact was not published
- Browser console logs CORS preflight failures to GitHub API endpoints
- CI changes are declared fixed without checking production Pages render
- Image-status ages are derived from workflow/run activity instead of GHCR package tags or release metadata.
- A dashboard card shows a numeric value without a traceable source URL/evidence path.
- A page-level JSON contract omits row-level provenance or hides missing values by dropping rows.
- Large inline Python or bash blocks (exceeding ~15 lines) are nested in workflow YAML, making testing and linting painful.
- Automated workflows that commit/push back to the git repository lack a concurrency limit block, causing push race conditions.
- **A single CI run hangs indefinitely on a private-network call; the concurrency group is starved** — missing timeout on an unreachable endpoint.
- **Tests fail in CI but pass locally** — the test environment diverges from runner environment (e.g. expects homelab LAN data that GitHub runners cannot reach). Without fallback handling and environment-aware assertions, this creates phantom CI failures that don't reflect real bugs.
- A new NodePort or Ingress manifest is proposed just to let a collector reach a service/pod that already has a ClusterIP or a diagnostics port — the API server proxy subresource reaches it without new cluster state.
- A dashboard row reports "bytes used" for storage that is actually a fixed-size preallocated block device, without noting the value is a derived estimate.

## Verification

- [ ] Workflow logic preserves last known live snapshot when private endpoint fetch fails
- [ ] `_meta.live_snapshot_ok` and `_meta.refreshed_at` are present and updated
- [ ] GitHub Pages source/build state was checked before declaring a site live
- [ ] If Cloudflare fronts the site, the live HTML keeps the raw dashboard script tag
- [ ] Browser fetch code avoids unnecessary custom headers that trigger preflight
- [ ] Production `https://factory.projectbluefin.io/` renders with real table/cluster content (no loading placeholders)
- [ ] Render validation includes a real browser run (headless is fine) and captures evidence
- [ ] Every `execSync`, `fetch`, or network call to private endpoints (`192.168.1.x`, internal IPs) has an explicit timeout set
- [ ] Build-time test assertions accept both live-environment data AND fallback/degraded paths (never assert presence of unreachable data)
- [ ] After fixing network timeouts or assertions, a CI run completed within 10 minutes with no hanging steps
- [ ] Image-status cards derive age from GHCR package tag publish/update timestamps when available, otherwise release `published_at`, and link to exact evidence URLs.
- [ ] Unsupported metrics (no source-of-truth feed) are hidden or explicitly unavailable, never synthesized.
- [ ] Page-level dashboard JSON keeps stable row keys plus row-level provenance/state fields so collector-only follow-up work can populate data without changing the contract.
- [ ] Inline Python/bash blocks over 15 lines are extracted to standalone script files under `scripts/`.
- [ ] Concurrency blocks are added to git-mutating workflows to secure the git-push transaction.
- [ ] Collectors reaching ClusterIP-only services or pod-only diagnostics ports use `kubectl get --raw .../proxy/...` instead of adding new NodePorts/manifests.
- [ ] Derived/estimated usage numbers (no direct source gauge) state the derivation formula and inputs in the row's `derivation` field.
