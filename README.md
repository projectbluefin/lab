# Lab

GitOps k3s cluster on `ghost` + `exo-0`, built on CNCF projects.

## Purpose

Internal lab for feature development before sending PRs upstream. Three purposes,
all built on the 40 Gbps point-to-point USB4 link between `ghost` and `exo-0`
(`thunderbolt0`, `10.99.0.0/30`). Policy routing (table 40, rule 5209) sends
cross-node pod traffic over the link; control-plane and DNS stay on Ethernet.
Clients on the LAN submit work as Argo Workflows (`http://192.168.1.102:32746`).

| Purpose | What runs | USB4 role | Status |
|---|---|---|---|
| **BuildStream build farm** | BuildBarn (`buildbarn` ns) plus the dakota OCI pipeline: `dakota-build-pipeline` / `bluefin-server-build-pipeline` build through BuildBarn, publish to the Zot registry (`:30500`), run Dakota QA and boot tests, and promote Zot candidates. Makes `projectbluefin/dakota`. | Hard admission gate: remote-execution actions and CAS transfers run pod-to-pod over the link; no Ethernet or local fallback. | Live |
| **Local LLM inference** | llama.cpp on Vulkan on `exo-0` (`manifests/llm-d.yaml`). OpenAI-compatible API at `http://192.168.1.102:30800/v1`. | Carries client and in-cluster traffic to the model server on `exo-0`. | Live |
| **Distributed ffmpeg encoding** | Plain Workflows from client tools (`tools/farm.py` in the video repos): frame-grid segments, parallel ffmpeg, `concat -c copy` join, ffprobe verify. | Chunk exchange between the two nodes' pods. | Live per node; cross-node chunking over USB4 not built yet |

## Stack

| Layer | Project | Role |
|---|---|---|
| Kubernetes | [k3s](https://k3s.io) | Local cluster |
| VM workloads | [KubeVirt](https://kubevirt.io) | Ephemeral boot-test VMs (`bluefin-server-boot-test`) |
| CI/CD | [Argo Workflows](https://argoproj.github.io/argo-workflows/) | DAG pipeline orchestration |
| GitOps | [Argo CD](https://argo-cd.readthedocs.io) | Declarative cluster state from git |
| Control plane | [KubeStellar](https://kubestellar.io) | WDS/ITS control plane for the local lab |
| Administration | [KubeStellar Console](https://github.com/kubestellar/console) | Sole private cluster-admin and single-pane UI |

## Cluster Topology

| Host | Role | Specs |
|---|---|---|
| ghost | k3s control-plane + KubeVirt compute | Ryzen AI MAX+ 395, 16c/32t, 64GB RAM |
| exo-0 | k3s worker | Framework Desktop |

| Namespace | Purpose |
|---|---|
| `argo` | Argo Workflows |
| `argocd` | ArgoCD controller |
| `buildbarn` | BuildBarn remote execution + CAS |
| `llm-d` | Local LLM inference (llama.cpp on Vulkan) |
| `local-registry` | Zot writable registry (30500) + pull-through cache (30501) |

## GitOps Model

| Application | Syncs path | Namespace |
|---|---|---|
| `lab` | `argo/workflow-templates/` | argo |
| `lab-infra` | `manifests/` | argo (+ others) |

1. Edit `argo/workflow-templates/` or `manifests/` → push to `main` → ArgoCD reconciles.
2. **Never** `kubectl apply` or `argo create workflow-template` for tracked resources — ArgoCD overwrites them.
3. `argo/bootstrap/` is not synced — run once by hand during cluster setup.

## Getting Started

See [docs/ops/bootstrap.md](/docs/ops/bootstrap.md) for full setup.

```bash
just setup-argocd                 # once
just argocd-sync
just run-bst-build                # dakota BST build via BuildBarn
just run-dakota-qa                # Dakota QA
just run-bluefin-server-build     # bluefin-server BST build
just run-zot-promotion ...        # promote a Zot candidate
just list-workflows
```

## Key Design Decisions

**No persistent VMs** — boot-test VMs are ephemeral and destroyed via `onExit`;
`just list-vms` shows zero VMs when no boot test is running.

**API-only operator model** — cluster reads and mutations go through the
Kubernetes API (MCP tools or `just` wrappers). No SSH to cluster nodes.

**One administration pane** — KubeStellar Console. No Grafana or parallel
dashboard framework.

**WorkflowTemplate over inline DAG** — reusable pipeline logic lives in
`argo/workflow-templates/`; ArgoCD owns the template lifecycle.

## Documentation Map

| Doc | Purpose |
|---|---|
| [AGENTS.md](AGENTS.md) | Agent entry point |
| [docs/reference/workflow-reference.md](/docs/reference/workflow-reference.md) | WorkflowTemplate submit interface and reference |
| [docs/ops/bootstrap.md](/docs/ops/bootstrap.md) | Replicate this lab from scratch |
| [docs/ops/RUNBOOK.md](/docs/ops/RUNBOOK.md) | Architecture + failure modes |
| [docs/reference/agent-cheatsheet.md](/docs/reference/agent-cheatsheet.md) | Command reference |
| [docs/ops/lab-operations.md](/docs/ops/lab-operations.md) | Operator procedures |
