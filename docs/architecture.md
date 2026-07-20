# Architecture

This repository contains declarative infrastructure for automated image
validation. Git-tracked manifests and workflow templates are reconciled by the
cluster's GitOps controller; workflows execute tests and publish structured
results consumed by the dashboard.

## Source of truth

- Workflow behavior: `argo/workflow-templates/`
- Cluster resources: `manifests/` and `argocd/`
- One-time setup: `argo/bootstrap/`
- Operator recipes: [`../Justfile`](../Justfile)
- Workflow contracts: [`reference/WORKFLOWS.md`](reference/WORKFLOWS.md)
- Durable design decisions: [`adr/README.md`](adr/README.md)

Do not infer live state from this overview. Inspect the manifest, workflow, or
published result that owns the fact.

## Reconciliation boundary

Managed resources are changed through version control and GitOps
reconciliation. Bootstrap resources are intentionally separate from managed
resources. The applicable procedure and verification commands are in
[`ops/README.md`](ops/README.md) and the relevant skill.

## Operational detail

Failure modes and recovery procedures belong in [`ops/RUNBOOK.md`](ops/RUNBOOK.md),
not in this overview. Current release evidence belongs in the generated
publisher data and its documented contract, not in this file.
