# Contributing

## What this repo is
`lab` is the owner's internal lab: k3s, Argo Workflows, ArgoCD, KubeVirt, and supporting manifests for the BuildStream build farm, local LLM inference, and distributed ffmpeg encoding.

## Prerequisites
- `kubectl`
- `argocd` CLI
- access to the test cluster, or a local QEMU/KubeVirt setup you can validate against
- familiarity with the GitOps flow in `/AGENTS.md`

## Development workflow
- Read `/AGENTS.md` and `/docs/reference/agent-cheatsheet.md` first
- Make infra changes in `argo/workflow-templates/`, `manifests/`, or `argocd/`
- Validate locally with:
```bash
just lint
actionlint .github/workflows/*.yml .github/workflows/*.yaml
```
- Commit to `main`; ArgoCD reconciles from `main`
