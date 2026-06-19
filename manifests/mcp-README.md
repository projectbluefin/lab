# Cluster MCP servers

In-cluster MCP server that lets agents (Claude Code, etc.) drive the lab
without SSH to ghost and without a local kubeconfig.

| Server | URL | Backing project |
|---|---|---|
| `k8s` | http://192.168.1.102:32767/sse | [containers/kubernetes-mcp-server](https://github.com/containers/kubernetes-mcp-server) |

Argo Workflows is driven through the same server — its ClusterRole grants
`create/delete` on `Workflows` (ad-hoc runs), `patch/update` on `CronWorkflows`
(suspend/resume), and read-only on `WorkflowTemplates` (GitOps-owned).
No separate Argo MCP server is needed.

## Two MCP interfaces — which one am I using?

| Interface | Tool prefix | Auth | Who uses it |
|---|---|---|---|
| **In-cluster SSE server** (this file) | `kubernetes-mcp-*` / `argo-mcp-*` | scoped ClusterRole | Claude Code, Cursor, any SSE client |
| **Pi native tools** (`argo_*` / `k8s_*`) | `argo_*` / `k8s_*` | local kubeconfig | Pi agent sessions |

The cheatsheet (`docs/agent-cheatsheet.md`) is written for the in-cluster SSE
interface. Pi session agents use the pi-native tool names (e.g. `argo_list_workflows`
instead of `argo-mcp-list_workflows`). Both interfaces reach the same cluster;
only the RBAC scope differs.

## Register with Claude Code

```sh
claude mcp add --transport sse k8s http://192.168.1.102:32767/sse
claude mcp list  # `k8s` should report ✓ Connected
```

## Permissions model

Scoped ClusterRole — no cluster-admin. RBAC:

- core/apps/batch: read-only (pods, services, deployments, jobs, events, ...)
- argoproj.io:
  - `Workflow`: create/delete (submit + clean up ad-hoc runs)
  - `WorkflowTemplate`: read-only (edits go via GitOps only)
  - `CronWorkflow`: patch/update (suspend/resume); create/delete are GitOps-owned
- kubevirt.io:
  - `VirtualMachine`: delete (orphan cleanup)
  - `VirtualMachineInstance`: read-only (controller-managed)

Tighten further once usage patterns are clear — see ADR 0001
(`docs/adr/0001-homelab-scale-cncf-minimalism.md`).

## Why no Argo-native MCP server?

The available Argo Workflows MCP servers (`Heapy/argo-workflows-mcp`,
`kushthedude/argo-workflows-mcp`) are **stdio-only** — they're designed to be
launched per-session as a local container, not deployed as a cluster Service.
Wrapping one in a stdio→SSE bridge is more moving parts than the
kubernetes-mcp-server's built-in CRD coverage warrants. Revisit if an
SSE-native Argo MCP server appears.
