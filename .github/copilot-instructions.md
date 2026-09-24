# Testing Lab Copilot Instructions

Use [`../AGENTS.md`](/AGENTS.md) for repo policy and architecture, and use [`../docs/agent-cheatsheet.md`](/docs/reference/agent-cheatsheet.md) for the canonical command reference.

Keep only these repo-specific inline reminders:

- Use `just` entrypoints first; do not duplicate command tables here.
- No SSH to ghost or exo-0.
- KubeStellar Console is the sole private cluster-admin/single-pane UI for the
  canonical local `ghost` k3s topology; Grafana or parallel dashboard frameworks
  are out of scope.
- No `kubectl apply` for `argo/workflow-templates/` or `manifests/`; edit git-tracked YAML and let ArgoCD reconcile it.
- VM-backed boot tests use ephemeral KubeVirt VMs — no persistent VMs. Container-only
  Dakota QA does not create VMs; `just list-vms` should show empty when no VM
  workflows run.
- After pushing a fix, verify the live template via `argo-mcp-get_workflow_template` before resubmitting — templates snapshot at submit time.
- Never enable shell tracing in Argo scripts that call authenticated APIs.
