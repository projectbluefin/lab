# Ubiquitous language

Glossary of canonical terms for the Project Bluefin factory dashboard and QA lab.
Terms are added as they are resolved; this file is a glossary only — no implementation details.

| Term | Meaning |
| --- | --- |
| **Factory** | The org-wide OS delivery system: image builds and releases across `projectbluefin/bluefin`, `bluefin-lts`, `dakota`, and `common`, plus their CI pipelines. The dashboard's primary subject. |
| **Lab** | This repo's QA cluster (Argo Workflows + KubeVirt). Additive to the Factory — lab data supplements factory data on the dashboard and is always marked as lab-sourced, never presented as factory health itself. |
| **Lane** | A `(variant, branch)` pair, e.g. `bluefin-testing`, `bluefin-lts-stable`. The unit of tracking for freshness, adoption, and test coverage. |
| **Release verdict** | Per-lane judgment of the latest published digest: **good** iff (1) the publishing build succeeded, (2) the lab QA pipeline passed against that exact digest, and (3) cosign signature verification passes. Security regression (new critical/high CVEs vs the previous digest) is displayed alongside the verdict but does not gate it. |
| **Countme (first-party)** | The `countme.projectbluefin.io` Cloudflare Worker plus weekly-salted HMAC ping clients shipped in bluefin, bluefin-lts, and dakota. Canonical adoption signal for the dashboard once an aggregate read path exists. Distinct from **Fedora countme** (upstream DNF infrastructure), which is a secondary series. |
| **Row contract** | The provenance shape every dashboard data row must carry: `source_url`, `collected_at`, `derivation`, `state`, `state_reason`. Defined in `docs/reference/page-contracts.md`. Placeholders are `null` + `state: unavailable` — collectors never invent values. |
