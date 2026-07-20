# Repository Agent Instructions

## Purpose

This repository defines and operates an automated Linux image testing lab. It
contains declarative cluster configuration, workflow definitions, test
execution, result publication, and dashboard source data.

Treat repository files as production infrastructure. Prefer the smallest
change that satisfies the task.

## Start here

1. Read this file.
2. Read [`docs/skills/README.md`](docs/skills/README.md).
3. Load only the skill that matches the files or behavior being changed.
4. Read [`docs/reference/ubiquitous-language.md`](docs/reference/ubiquitous-language.md)
   when a project term is ambiguous.
5. Read [`docs/ops/RUNBOOK.md`](docs/ops/RUNBOOK.md) when diagnosing an
   operational failure.
6. Inspect the source workflow, manifest, or script before changing related
   documentation.

## Task routing

| Task | First document |
|---|---|
| Workflow or WorkflowTemplate change | [`docs/skills/argo-workflows/SKILL.md`](docs/skills/argo-workflows/SKILL.md) |
| GitOps reconciliation or manifest change | [`docs/skills/gitops-argocd/SKILL.md`](docs/skills/gitops-argocd/SKILL.md) |
| Cluster add-on, storage, or node issue | [`docs/skills/cluster-tooling/SKILL.md`](docs/skills/cluster-tooling/SKILL.md) |
| Virtual machine lifecycle or boot failure | [`docs/skills/kubevirt-vms/SKILL.md`](docs/skills/kubevirt-vms/SKILL.md) |
| Test authoring or test failure | [`docs/skills/test-authoring/SKILL.md`](docs/skills/test-authoring/SKILL.md) |
| Dashboard page or data contract | [`docs/skills/astro-dashboard-pages/SKILL.md`](docs/skills/astro-dashboard-pages/SKILL.md) |
| CI workflow or validation failure | [`docs/skills/ci-tooling/SKILL.md`](docs/skills/ci-tooling/SKILL.md) |
| Documentation or skill maintenance | [`docs/skills/meta-skill-improvement/SKILL.md`](docs/skills/meta-skill-improvement/SKILL.md) |

## Validation

Run the lightest relevant checks first, then the full applicable checks.

```bash
just lint
python3 scripts/validate-docs.py
```

When dashboard source or dependencies change:

```bash
npm ci
npm run build
```

When changing workflow or manifest YAML, run the repository YAML and workflow
validation documented by the applicable skill before requesting review.

## Repository boundaries

- Do not apply managed WorkflowTemplates directly to the cluster.
- Do not bypass the declared GitOps reconciliation path.
- Do not commit credentials, tokens, private keys, or host-specific secrets.
- Do not commit transient logs, session notes, generated caches, or stale
  screenshots unless the generation workflow requires them.
- Do not edit generated dashboard output by hand.
- Do not place one-off incident notes in evergreen documentation.
- Do not duplicate facts that already have a canonical reference document.
- Treat nested repositories as separate ownership boundaries and follow their
  nearest agent instructions.

## Documentation rules

- Use standard CommonMark/GFM Markdown and relative links.
- Put durable procedures in `docs/skills/` or `docs/ops/`.
- Keep `SKILL.md` files concise; defer large references to each skill's
  supporting files.
- Update the relevant skill when a change reveals a reusable pattern.
- Keep public documentation generic and free of client-specific framing.
- Record durable decisions in an ADR rather than burying them in a runbook.

## Completion checklist

Before handoff:

- [ ] The relevant skill was loaded.
- [ ] The source of each changed fact was checked.
- [ ] Applicable validation commands passed.
- [ ] Links and file references resolve.
- [ ] No secrets or transient artifacts were added.
- [ ] Durable new knowledge was written to the relevant documentation.
