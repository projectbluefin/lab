---
name: ci-tooling
description: >
  GitHub Actions workflow authoring and debugging for lab automation. Use when
  changing .github/workflows/ or wiring CI jobs that need private-cluster data.
metadata:
  context7-sources:
    - /websites/github_en_actions
    - /websites/github_en_rest
    - /websites/git-scm
---

# CI Tooling — GitHub Actions in lab

## When to Use

- Editing `.github/workflows/*.yml`
- A workflow needs homelab/private network data

## When NOT to Use

- Argo WorkflowTemplate logic in `argo/workflow-templates/` (use `argo-workflows.md`)
- ArgoCD reconciliation policy work (use `gitops-argocd.md`)
- VM lifecycle/scheduling behavior (use `kubevirt-vms.md`)

## Core Process

1. Confirm runner network model first: GitHub-hosted runners have public internet by default; private network access requires an overlay/VPN setup or a self-hosted runner.
2. Extract large inline scripts (especially Python/bash blocks over ~10-15 lines) from GHA YAML workflow files into standalone executable scripts under `scripts/`. This enables independent local execution, linting, testing, and modular maintenance.
3. Configure explicit GHA concurrency limits (`concurrency:`) on any automated workflow that commits/pushes files back to git. Use a unique group name (e.g. `group: derive-${{ github.ref }}`) and set `cancel-in-progress: true` to prevent race conditions and rebase conflicts when multiple runs trigger in rapid succession.
4. **Network timeouts are mandatory for any cluster-facing call in CI**: every `execSync`, `fetch`, `curl`, `skopeo`, or similar command that reaches `<lab-subnet>.x` or any private endpoint must have an explicit timeout (e.g. `timeout: 2000` for Node.js, `--max-time` for curl, or socket timeout). Without timeouts, a single unreachable endpoint will hang the entire GitHub-hosted runner indefinitely, starving the concurrency group and blocking all downstream runs.
5. **Build-time assertions must handle environment drift**: if a test asserts presence of specific live data (e.g. `:30501` registry port), but the build environment differs from the test environment (GitHub runners → no homelab network access), the assertion will always fail. Instead: allow the code to handle missing data gracefully (e.g. fallback rendering), and update the assertion to accept both the live path and the fallback path. This prevents false CI failures that don't reflect real code bugs.
6. **Prefer the Kubernetes API server's service/pod proxy subresource over new NodePorts/manifests** when a collector needs to reach a ClusterIP-only service or a pod's non-Service-exposed diagnostics port: `kubectl get --raw "/api/v1/namespaces/<ns>/services/<svc>:<port>/proxy/<path>"` or `.../pods/<pod>:<port>/proxy/<path>`. This routes through the API server the runner already reaches (the same reachability `kubectl get nodes` relies on), so no cluster manifest changes are needed to expose a new metrics/status endpoint.
7. **Git-mutating workflow publishers must keep tokens out of URLs and argv.** Use a credential-free HTTPS remote plus `GIT_ASKPASS`/`GIT_TERMINAL_PROMPT=0`, with the token supplied only through the process environment. If an append-only history push loses a race, fetch/reset a disposable clone to the latest remote branch and replay the validated append before retrying; never force-push or resolve the NDJSON conflict with `-X ours`, because either can discard a concurrent record. Source: `/websites/git-scm`.

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "A green `just lint` means the required `lint` check will pass." | CI's actionlint runs shellcheck; backticks or `$` inside single quotes trip SC2016. Run `actionlint` directly. |

## Red Flags

- Large inline Python or bash blocks (exceeding ~15 lines) are nested in workflow YAML, making testing and linting painful.
- Automated workflows that commit/push back to the git repository lack a concurrency limit block, causing push race conditions.
- **A single CI run hangs indefinitely on a private-network call; the concurrency group is starved** — missing timeout on an unreachable endpoint.
- **Tests fail in CI but pass locally** — the test environment diverges from runner environment (e.g. expects homelab LAN data that GitHub runners cannot reach). Without fallback handling and environment-aware assertions, this creates phantom CI failures that don't reflect real bugs.
- A new NodePort or Ingress manifest is proposed just to let a collector reach a service/pod that already has a ClusterIP or a diagnostics port — the API server proxy subresource reaches it without new cluster state.
- A workflow embeds a token in its clone URL/command arguments, force-pushes generated history, or uses `-X ours` on append-only NDJSON.
- A publishing workflow has been red for dozens of consecutive runs and nobody has checked whether a ruleset was created around the last green run (`gh api repos/<owner>/<repo>/rulesets`, compare `created_at` to the last success).

## Verification

- [ ] Every `execSync`, `fetch`, or network call to private endpoints (`<lab-subnet>.x`, internal IPs) has an explicit timeout set
- [ ] A publishing workflow was verified by an **unattended scheduled run**, not only a `workflow_dispatch`, and `main` actually advanced
- [ ] `actionlint` was run directly (not only `just lint`) before opening the PR
- [ ] Build-time test assertions accept both live-environment data AND fallback/degraded paths (never assert presence of unreachable data)
- [ ] After fixing network timeouts or assertions, a CI run completed within 10 minutes with no hanging steps
- [ ] Inline Python/bash blocks over 15 lines are extracted to standalone script files under `scripts/`.
- [ ] Concurrency blocks are added to git-mutating workflows to secure the git-push transaction.
- [ ] Collectors reaching ClusterIP-only services or pod-only diagnostics ports use `kubectl get --raw .../proxy/...` instead of adding new NodePorts/manifests.
- [ ] Git publishers use credential-free remotes plus non-interactive credential helpers, and replay append-only updates from the latest remote state after a push race.
