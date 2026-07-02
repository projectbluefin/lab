# Upstream Dashboard and Cluster Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the upstream dashboard page to match the newer dashboard patterns and make the overview cluster status display the actual node readiness state from the existing source-of-truth data.

**Architecture:** Rework the `/upstream/` page layout to follow the same card-based dashboard structure used elsewhere in the site, and wire the overview cluster cards to the canonical factory stats data so they render the current node readiness state rather than stale or fabricated values.

**Tech Stack:** Astro, JSON data snapshots, CSS, existing dashboard page patterns.

## Global Constraints

- Preserve the existing dashboard data contract and source-of-truth flow.
- Keep the cluster status rendering sourced from the canonical stats JSON rather than introducing a one-off hardcoded override.
- Validate the Astro site with the existing build and test commands.

---

### Task 1: Refresh the upstream page layout

**Files:**
- Modify: `src/pages/upstream.astro`
- Modify: `src/styles/site.css` (if shared classes are needed)

**Interfaces:**
- Consumes: the existing `buildUpstreamPageModel` model and upstream status JSON.
- Produces: a more polished `/upstream/` experience that mirrors the other dashboard pages.

- [ ] **Step 1: Review the existing upstream page and the other dashboard page patterns**

Use `src/pages/bluefin.astro` and `src/pages/index.astro` as the reference layout, and capture the sections that should be preserved for the general upstream page: summary metrics, family cards, chart cards, and stream details.

- [ ] **Step 2: Replace the legacy page structure with the shared dashboard card pattern**

Update `src/pages/upstream.astro` to use the newer card-based layout and headings similar to the other dashboard pages, keeping the existing data model and evidence links intact.

- [ ] **Step 3: Verify the page still renders with the shared styles**

Run the Astro build and ensure the page compiles without missing classes or broken markup.

### Task 2: Make cluster node readiness reflect the canonical data snapshot

**Files:**
- Modify: `src/pages/index.astro`
- Possibly modify: `docs/data/factory-stats.json` via the existing refresh workflow if the current snapshot is stale
- Possibly modify: `scripts/refresh_factory_stats.py` only if the existing logic is missing a required field

**Interfaces:**
- Consumes: `stats.factory.cluster.nodes` from the generated dashboard stats JSON.
- Produces: cluster cards that visibly reflect the current readiness state from the canonical data source.

- [ ] **Step 1: Inspect the current factory stats generator and the generated JSON**

Confirm the dashboard uses `docs/data/factory-stats.json` and that the node status values are generated from the live cluster snapshot logic in `scripts/refresh_factory_stats.py`.

- [ ] **Step 2: Refresh the stats JSON from the canonical source**

Run the existing refresh script or the repository’s documented workflow to regenerate the dashboard stats so the cluster node status reflects the current cluster state rather than a stale snapshot.

- [ ] **Step 3: Update the overview page rendering to surface the node status clearly**

Adjust the node cards on `src/pages/index.astro` to show a status label with explicit tone classes for ready vs not-ready nodes, and ensure the card’s content makes that status obvious.

- [ ] **Step 4: Validate the dashboard build and page tests**

Run the existing build and test commands to confirm the dashboard still renders and that the status display behaves correctly.
