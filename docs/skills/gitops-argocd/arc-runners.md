---
name: arc-runners
description: >
  ArgoCD-managed GitHub Actions Runner Controller (ARC) in the lab.
---

## 7. OCI Helm chart Applications (arc-systems, arc-runners)

ArgoCD can deploy OCI Helm charts directly. These Applications live under `argocd/`
and are applied once as control-plane resources (not GitOps-managed by ArgoCD itself).

```bash
# Apply ARC ArgoCD Applications (one-time, or after cluster rebuild)
kubectl apply -f argocd/arc-controller-app.yaml -n argocd
kubectl apply -f argocd/arc-runners-app.yaml -n argocd
```

**CRD annotation size limit** — Large CRDs (e.g. `autoscalingrunnersets.actions.github.com`)
exceed ArgoCD's 262KB client-side `last-applied-configuration` annotation limit.
`ServerSideApply=true` is set in `argocd/arc-controller-app.yaml`, but the CRD may
still appear `SyncFailed` / `OutOfSync` in the ArgoCD UI. The CRD itself is applied
and functional; the error is cosmetic once the CRD reaches `Healthy`. Verify with:
```bash
kubectl get crd autoscalingrunnersets.actions.github.com
kubectl get autoscalingrunnersets -n arc-runners
```

**GitHub App secret** — `arc-runners` needs `arc-github-secret` containing
`github_app_id`, `github_app_installation_id`, and `github_app_private_key`.
Without it the controller logs `failed to find GitHub config secret` and no
listener or runner pods are created.

A contributor with gh CLI auth and the downloaded private key can recreate it:
```bash
./scripts/setup-arc-github-secret.sh
```
The script fetches App/Installation IDs from the GitHub API and prompts for the
private key file. The private key itself must be generated from the GitHub App
settings UI; it cannot be retrieved via API.

**Stuck retry loop** — if ArgoCD retries a failed sync with stale syncOptions:
```bash
kubectl patch application <name> -n argocd \
  --type=json -p='[{"op":"remove","path":"/operation"}]'
kubectl annotate application <name> -n argocd \
  argocd.argoproj.io/refresh=hard --overwrite
```

**Controller service account discovery** — `gha-runner-scale-set` discovers the
controller SA by label lookup. Fails when controller and runners are in different
namespaces. Always set explicitly in helm values:
```yaml
controllerServiceAccount:
  namespace: arc-systems
  name: arc-systems-gha-rs-controller
```

**worker scheduling** — workflow pods may land on any online worker. If a pod lands on
an unhealthy worker and fails, delete it so Kubernetes can reschedule it to a healthy node.

