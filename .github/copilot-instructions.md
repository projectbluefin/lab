# Testing Lab Copilot Instructions

Load these docs in this order:

1. [`docs/agent-cheatsheet.md`](../docs/agent-cheatsheet.md) for the canonical command matrix and routine recipes
2. [`AGENTS.md`](../AGENTS.md) for repo policy, scope gates, and architecture tables
3. [`docs/lab-operations.md`](../docs/lab-operations.md) for long-form decision trees
4. [`RUNBOOK.md`](../RUNBOOK.md) for architecture and failure-mode context

Repo-specific habits:

- Use `just` entrypoints first; do not duplicate command tables here.
- No SSH to ghost or exo-1.
- No `kubectl apply` for `argo/workflow-templates/` or `manifests/`; edit git-tracked YAML and let ArgoCD reconcile it.
- Prefer titan workflows for test-only iteration and fresh-VM workflows for image or golden-disk validation.
- PR queue work is only complete with real lab evidence in [`docs/vanguard-report-template.md`](../docs/vanguard-report-template.md).
- Titan `authorized_keys` refresh remains human-gated; if titan SSH breaks after key rotation, file an issue instead of patching the host manually.
