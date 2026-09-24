# Lab — Agent Entry Point

Internal lab for the owner's feature development before sending PRs upstream.
Argo Workflows, ArgoCD, KubeVirt, and the manifests for the `ghost` + `exo-0`
k3s cluster.

## Start here

1. Read this file.
2. Find the skill for the area you need in [`docs/SKILL.md`](docs/SKILL.md)
   (the skill router, same convention as `projectbluefin/common`) and load
   only that skill.
3. For deterministic commands, check the [`Justfile`](Justfile) or
   [`docs/reference/agent-cheatsheet.md`](docs/reference/agent-cheatsheet.md).
4. For operational failure modes, read [`docs/ops/RUNBOOK.md`](docs/ops/RUNBOOK.md).
5. Before changing anything, check the latest project context in
   [`docs/reference/ubiquitous-language.md`](docs/reference/ubiquitous-language.md).

## What this repo is

Three purposes, all on the `ghost`<->`exo-0` USB4 link:

- **BuildStream build farm** — BuildBarn + BST pipelines (dakota,
  bluefin-server), the Zot registry, and the dakota OCI pipeline that makes
  `projectbluefin/dakota` (Dakota QA, boot tests, Zot candidate promotion).
- **Local LLM inference** — llm-d (llama.cpp on Vulkan on `exo-0`, NodePort
  30800, OpenAI-compatible `/v1`).
- **Distributed ffmpeg encoding** — plain Workflows from client tools
  (`tools/farm.py` in the video repos), plus `images/video-upscale`.

Internal only: not Project Bluefin's public CI/QA. KubeStellar Console is the
sole private cluster-administration UI.

## Build / test / lint

```bash
just lint              # actionlint + argo lint + registry allowlist
```

## Boundaries

- The owner commits straight to `main`; no PR or merge-queue flow.
- Do not `kubectl apply` WorkflowTemplates — ArgoCD owns them.
- **Always prefer industry-standard CNCF/OCI tooling over distribution tooling.**
  This is the project's greatest competitive advantage. Compose capability from
  [`fsdk-containers`](https://github.com/projectbluefin/fsdk-containers); if the
  image you need is missing, propose adding one.
- **Do not use RPM or `dnf` for image composition or package installation** —
  not at runtime, not in a Containerfile, not in a builder stage. No SRPM, Packit,
  or RPM build paths exist in this lab. Fetch upstream release artifacts by checksum
  instead. See [`docs/skills/gitops-argocd/image-policy.md`](docs/skills/gitops-argocd/image-policy.md).
- **Never use a distro-packaged `ffmpeg`.** Fedora's `ffmpeg-free` is
  patent-stripped (measured on fc44: no `libx265`, no `libsvtav1`), so an
  encode path fails hours into a render. `ghcr.io/projectbluefin/bluefin` ships
  a full org-built ffmpeg.
- Do not introduce Grafana or another general-purpose cluster-admin/dashboard
  framework alongside KubeStellar Console.
- Treat the local `ghost` k3s topology as canonical; do not design lab
  automation around cloud or external multi-cluster assumptions.
- Do not SSH into cluster nodes from a workstation; CLI access is via `just`,
  `argo`, and `kubectl`.
- Do not commit transient session artifacts (`TODO-*.md`, `poll-*.log`,
  stale screenshots, etc.) to this repo.
- Cluster-specific hostnames, IPs, and one-off incident notes belong in private
  runbooks, not in this repository.
- Issue tracker for this repo is `projectbluefin/lab`.

## When to Use / When NOT to Use

| Task | Where to go |
|---|---|
| Which skill should I load? | [`docs/SKILL.md`](docs/SKILL.md) |
| Authoring Argo workflow templates (YAML) | [`docs/skills/argo-workflows/SKILL.md`](docs/skills/argo-workflows/SKILL.md) |
| KubeVirt VM provisioning / boot failures | [`docs/skills/kubevirt-vms/SKILL.md`](docs/skills/kubevirt-vms/SKILL.md) |
| ArgoCD sync, GitOps rules, bootstrap vs managed | [`docs/skills/gitops-argocd/SKILL.md`](docs/skills/gitops-argocd/SKILL.md) |
| Cluster add-ons, k3s, registries | [`docs/skills/cluster-tooling/SKILL.md`](docs/skills/cluster-tooling/SKILL.md) |
| KubeStellar core, WECs, BindingPolicies | [`docs/skills/kubestellar/SKILL.md`](docs/skills/kubestellar/SKILL.md) |
| KubeStellar Console deploy/auth/cards | [`docs/skills/console-dashboard/SKILL.md`](docs/skills/console-dashboard/SKILL.md) |
| Add/remove nodes or WECs, BST grid scaling | [`docs/skills/node-lifecycle/SKILL.md`](docs/skills/node-lifecycle/SKILL.md) |
| End of session write-back loop | [`docs/skills/meta-skill-improvement/SKILL.md`](docs/skills/meta-skill-improvement/SKILL.md) |
| Workflow parameter contracts | [`docs/reference/WORKFLOWS.md`](docs/reference/WORKFLOWS.md) |
| Architecture / failure modes | [`docs/ops/RUNBOOK.md`](docs/ops/RUNBOOK.md) |
| Human contributor workflow | [`CONTRIBUTING.md`](CONTRIBUTING.md) |

## Skill maintenance

At the end of any non-trivial session, update the skill file for the area you
changed if you learned something new. Every session produces two outputs: the
work and the learning.
