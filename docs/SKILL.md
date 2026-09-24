# Lab Skill Router

Agent entry point for `projectbluefin/lab`. Find the skill that matches your task, load only that skill, then act.

For factory onboarding and cross-repo rules, see [`AGENTS.md`](../AGENTS.md) and [`projectbluefin/common`](https://github.com/projectbluefin/common).

## Read order

1. [`AGENTS.md`](../AGENTS.md) — repo contract, build commands, boundaries.
2. This file (`docs/SKILL.md`) — task to skill mapping.
3. The specific skill file in `docs/skills/`.

## Skill index

| I need to... | Load |
|---|---|
| Author, lint, or debug Argo Workflows / WorkflowTemplates | [`skills/argo-workflows/SKILL.md`](skills/argo-workflows/SKILL.md) |
| Build or troubleshoot the `bluefin-server` bootc image | [`skills/bluefin-server/SKILL.md`](skills/bluefin-server/SKILL.md) |
| Author or debug GitHub Actions workflows | [`skills/ci-tooling/SKILL.md`](skills/ci-tooling/SKILL.md) |
| Manage cluster add-ons, registries, k3s, or external-secrets | [`skills/cluster-tooling/SKILL.md`](skills/cluster-tooling/SKILL.md) |
| Deploy/upgrade KubeStellar Console, auth policy, cards, or missions | [`skills/console-dashboard/SKILL.md`](skills/console-dashboard/SKILL.md) |
| Review Dakota PRs using the lab-backed QA workflow | [`skills/dakota-pr-review/SKILL.md`](skills/dakota-pr-review/SKILL.md) |
| Configure ArgoCD sync, GitOps rules, bootstrap vs managed | [`skills/gitops-argocd/SKILL.md`](skills/gitops-argocd/SKILL.md) |
| KubeStellar install/upgrade, WEC registration, BindingPolicies | [`skills/kubestellar/SKILL.md`](skills/kubestellar/SKILL.md) |
| Provision, manage lifecycle, or debug KubeVirt VMs | [`skills/kubevirt-vms/SKILL.md`](skills/kubevirt-vms/SKILL.md) |
| Run self-improvement loop, failure triage, or add new skills | [`skills/meta-skill-improvement/SKILL.md`](skills/meta-skill-improvement/SKILL.md) |
| Add/remove nodes or WECs, second-PC expansion, BST grid scaling | [`skills/node-lifecycle/SKILL.md`](skills/node-lifecycle/SKILL.md) |

For details and secondary skill topic guides, see [`docs/skills/README.md`](skills/README.md).
