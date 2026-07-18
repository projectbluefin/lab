# ADR 0002 — Release verdict definition

Status: Accepted
Date: 2026-07-18
Amends: ADR 0001 (re-admits cosign verification; acknowledges GitHub Actions CI)

## Context

The factory dashboard's primary job is answering an on-call SRE's question:
"is the last release good?" That question needs a precise, per-lane
definition — otherwise every page invents its own, and charts disagree.

A lane is a `(variant, branch)` pair (`bluefin-testing`,
`bluefin-lts-stable`, `dakota-testing`, ...). The Factory is org-wide
(bluefin, bluefin-lts, dakota, common); the Lab is additive evidence,
always marked as lab-sourced.

Two clauses of ADR 0001 block the pieces this verdict needs:

- ADR 0001 listed "Cosign signature verification, SBOM generation" as
  out-of-scope, and required a new ADR for re-introduction.
- ADR 0001 stated "No GitHub Actions, no `.github/workflows/` CI" — the
  repo has since accumulated five workflows (lint, CI, data refresh) that
  are load-bearing for the dashboard. That clause is dead in practice.

## Decision

For each lane, the latest published digest is **good** iff all three hold:

1. **Build succeeded** — the publishing workflow run for that digest
   concluded successfully (public GitHub Actions API).
2. **QA verdict passed** — the lab QA pipeline ran against that exact
   digest and passed. Lab evidence gates the verdict even though the lab
   is additive infrastructure: a release nobody has booted is not "good",
   and the per-digest lab run is the only boot evidence the factory has.
3. **Signature verifies** — `cosign verify` against the publisher's
   `cosign.pub` passes for the digest.

**Security regression does not gate the verdict.** New critical/high CVEs
versus the previous digest are displayed alongside the verdict as their
own trend. Rationale: a newly disclosed upstream CVE would flip a release
to "bad" when the previous release carries the same vulnerability and no
fix has shipped — that signal punishes releasing, which is backwards.

### ADR 0001 amendments

- **Cosign verification is re-admitted** — read-only consumption of
  publisher signatures. No key custody, no signing, no Rekor/sigstore
  deployment in the lab. Signing remains publisher-side in the image
  repos. The minimalism spirit holds: we consume signatures, we do not
  operate signature infrastructure.
- **GitHub Actions CI is accepted current practice** for repo lint and
  dashboard data collection. ADR 0001's "no `.github/workflows/` CI"
  clause is retired.

## Consequences

- Every dashboard surface that renders release health derives from this
  single definition; collectors emit the three inputs per lane with full
  row-contract provenance (`source_url`, `collected_at`, `derivation`,
  `state`, `state_reason`).
- A lane with a missing input (e.g., no lab run for the digest yet)
  renders an explicit `unavailable`/pending verdict — never inferred good.
- Changing what gates the verdict requires amending this ADR, not editing
  a collector.
