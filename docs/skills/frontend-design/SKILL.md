---
name: frontend-design
description: >
  Design direction for the Project Bluefin factory dashboard website. Use whenever
  modifying Astro pages, CSS, charts, or visual components under `src/`. Enforces
  deliberate, subject-grounded choices over templated dashboard defaults.
metadata:
  context7-sources:
    - /withastro/docs
    - /apache/echarts-doc
    - https://www.skills.sh/anthropics/skills/frontend-design
---

# Frontend Design

## Overview

The factory dashboard at `factory.projectbluefin.io` is the public face of an
operating-system QA lab: Argo Workflows, KubeVirt VMs, bootc images, and
behaviour-driven tests running on a homelab cluster. Every page is an evidence
surface, not a generic admin panel. Treat the site as a design brief with a
specific subject, audience, and job.

## When to Use

- Adding or revising pages under `src/pages/*.astro`.
- Editing global styles, layout components, or the design token set.
- Adding or restyling charts, tables, cards, or navigation.
- Choosing typography, colour, spacing, or motion for any site element.
- Replacing placeholder or templated visuals with subject-specific treatment.

## When NOT to Use

- Backend/collector changes in `.github/workflows/` or `scripts/` (use the
  matching infra or CI skill).
- Argo WorkflowTemplate or Kubernetes manifest changes (use `argo-workflows.md`
  or `kubevirt-vms.md`).
- Pure content updates that do not change visual design (for example fixing a
  typo in an existing paragraph).

## Core Process

1. **Pin the brief.** Before opening a design file, state the page's subject,
   audience, and single job. For this site the subject is always one of:
   image release freshness, test evidence, provisioning infrastructure, build
   lineage, application telemetry, or adoption metrics. The audience is a
   contributor or operator who needs to answer a specific question in under ten
   seconds. The job is to make the evidence scannable and trustworthy.
2. **Ground every choice in the subject.** Pull visual language from the
   Bluefin world: atomic updates, reflink copies, container disks, bootc
   images, GNOME Shell tests, Zot registry caches, USB4 mesh links. Use those
   concepts as metaphors for layout, colour, and motion instead of generic
   dashboard tropes.
3. **Hero as thesis.** The top of every page should answer the most important
   question first. A big number with a tiny label is the default; only use it
   when the number itself is the answer. Prefer a clear status statement, a
   small multiples grid, or a live chart that shows the characteristic thing
   about that page.
4. **Design the type, do not just set it.** The current site uses Inter as a
   neutral workhorse. When you add new elements, make deliberate choices about
   weight, tracking, case, and scale. Use uppercase labels and monospaced data
   deliberately, not as decoration. Keep line lengths comfortable for reading
   technical prose.
5. **Structure must carry information.** Numbered markers, eyebrows, dividers,
   and badges should encode real order or real status. Do not use "01 / 02 / 03"
   unless the items are a true sequence such as a pipeline stage. Prefer labels
   that state the relationship: "Source", "Assemble", "Verify", "Ship".
6. **Colour is status, not wallpaper.** The existing palette is dark with
   cyan/blue accents, green for healthy, amber for warning, rose for failure,
   and purple for cluster/fabric. New colours must earn their place. If you add
   a hue, assign it a semantic role and document it. Avoid gradients that do
   not encode data.
7. **Use motion with intent.** Motion should clarify state transitions, reveal
   evidence on demand, or mark live data. Avoid scattershot entrance animations.
   One orchestrated moment — a chart drawing itself on first view, a table row
   highlighting on hover, a status pulse — is usually enough.
8. **Match complexity to the page's vision.** Dense matrix pages (Tests, Builds,
   Images) need tight grids and high information density. Narrative pages (About)
   need generous whitespace and larger type. Do not apply the same card density
   everywhere.
9. **Prefer local assets and ESM bundles.** Load ECharts via `import * as echarts
   from 'echarts'` and page scripts from `src/scripts/`. Avoid CDN scripts so the
   site builds offline and Cloudflare Rocket Loader cannot rewrite boot paths.
10. **Test the design in built output.** A change is not done until `npm run
    build` succeeds and the rendered HTML contains the intended structure,
    wording, and chart containers. Use page smoke tests to lock the behaviour.

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "It is just a dashboard; neutral is fine." | Neutral is invisible. The site represents a distinctive operating-system factory; its visuals should reinforce that identity. |
| "I will reuse the same card style from the last page." | Reuse is fine, but blindly copying layout without asking what that page's job is produces generic pages. |
| "A numbered list looks more designed." | Numbers imply sequence. If the content is not ordered, use status labels, categories, or no marker at all. |
| "More animation makes it feel premium." | Scattered motion cheapens the experience. One justified animation beats five decorative ones. |
| "The data is the design." | Data needs a designed container. Poor spacing, weak hierarchy, and default chart styling undermine trust in the evidence. |
| "I can pull this library from a CDN." | The site is built for GitHub Pages and proxied through Cloudflare. Bundle dependencies locally to avoid runtime failures and Rocket Loader issues. |

## Red Flags

- Adding a new page that looks interchangeable with any other status dashboard.
- Introducing a colour that has no defined semantic role.
- Using numbered markers for non-sequential content.
- Loading external scripts or fonts without a local fallback.
- Hiding chart sections or data tables when state is missing instead of rendering
  an explicit unavailable block.
- Applying the same card density to every page regardless of its information
  architecture.
- Changing visual style without updating the page smoke test or checking the
  built HTML.
- Adding hero KPIs that do not answer the page's primary question.

## Verification

- [ ] The page's subject, audience, and single job are stated in the design
      rationale or PR description.
- [ ] Colour, type, and layout choices are justified by the subject matter, not
      copied from a generic dashboard.
- [ ] Any new colour has a documented semantic role (status, family, fabric,
      etc.).
- [ ] Structural markers (numbers, eyebrows, dividers) encode real information.
- [ ] Motion, if used, has a single clear purpose.
- [ ] ECharts and other JS dependencies are imported as ESM modules, not loaded
      from a CDN.
- [ ] `npm run build` succeeds and `npm test` passes for the affected pages.
- [ ] Built HTML shows no emojis and follows the dark colour-scheme contract.
