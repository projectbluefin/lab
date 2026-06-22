# Testing Lab Copilot Instructions

Use [`../AGENTS.md`](../AGENTS.md) for repo policy and architecture, and use [`../docs/agent-cheatsheet.md`](../docs/agent-cheatsheet.md) for the canonical command reference.

Keep only these repo-specific inline reminders:

- Use `just` entrypoints first; do not duplicate command tables here.
- No SSH to ghost or exo-1.
- No `kubectl apply` for `argo/workflow-templates/` or `manifests/`; edit git-tracked YAML and let ArgoCD reconcile it.
- All test runs use ephemeral KubeVirt VMs — no persistent titan VMs. `just list-vms` should show empty when no workflows run.
- After pushing a fix, verify the live template via `argo-mcp-get_workflow_template` before resubmitting — templates snapshot at submit time.
- PR queue work is only complete with real lab evidence.
