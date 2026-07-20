---
name: agent-cheatsheet-arc-runners
description: >
  ARC runner notes extracted from the agent cheatsheet.
---

# ARC Runners (GitHub Actions on ghost)

When no jobs are queued, `arc-runners` namespace is empty — that is correct.
Runners are ephemeral and only exist while a job is running.

`ghost-runners` uses **container mode (`type: kubernetes`)**: the runner
controller/listener pod is small, and each GitHub Actions job runs as a separate
Kubernetes pod. Heavy work should be submitted to the cluster as an Argo
Workflow; that keeps the runner tiny while the actual build pods use full
cluster resources.

**Check ARC is healthy:**
```bash
kubectl get pods -n arc-systems
```
Expected: `arc-systems-gha-rs-controller-*` Running + `ghost-runners-*-listener` Running.

**Check a runner set is registered:**
```bash
kubectl get autoscalingrunnersets -n arc-runners
```
Expected: `ghost-runners` with MINIMUM=0 MAXIMUM=6.

**Check container-mode job pods:**
```bash
kubectl get pods -n arc-runners -w
```
A small ephemeral runner pod appears first; when a job runs, a second pod
(the step/job pod) is created from the `container:` image declared in the
workflow.

**If listener is missing** (arc-systems has only the controller pod, no listener):
1. Check controller logs: `kubectl logs -n arc-systems <controller-pod>`
2. If error is `no route to host` / DNS failure: the controller likely landed away from ghost.
   Delete the controller pod — it will reschedule to ghost where DNS works.
3. If error is GitHub API auth failure: check `arc-github-secret` exists in `arc-runners`.

**Trigger a workflow using ARC:**
Add `runs-on: ghost-runners` and a `container:` block to any projectbluefin workflow.
A listener pod and ephemeral runner pod will appear in `arc-systems` and
`arc-runners` respectively. Example: `.github/workflows/example-container-mode-build.yml`.
For maintainer access, authentication model, and troubleshooting, see
`/docs/ops/maintainer-onboarding.md`.

**Writing a container-mode job:**
- The job **must** declare `container:`; without it the runner will fail.
- Keep the step container image small-to-medium. Offload heavy builds to Argo
  Workflows using `argo submit --from workflowtemplate/<name> --wait`.
- The runner service account (`arc-runner-workflow-submitter`) can create and
  watch workflows in the `argo` namespace.

**Allow a maintainer to use ghost-runners on personal repos:**
The org `ghost-runners` scale set cannot serve repos outside `projectbluefin`.
Create a second scale set for the maintainer's personal account:

1. Maintainer installs the `bluefin-ghost-arc` GitHub App on their personal
   account and notes the installation ID.
2. Create a secret for the personal installation:
   ```bash
   kubectl create secret generic arc-github-secret-personal \
     --namespace arc-runners \
     --from-literal=github_app_id=4099840 \
     --from-literal=github_app_installation_id="<PERSONAL_INSTALLATION_ID>" \
     --from-literal=github_app_private_key="$(cat /path/to/bluefin-ghost-arc.pem)"
   ```
3. Add an ArgoCD Application like `argocd/arc-runners-personal-app.yaml` with
   `githubConfigUrl: https://github.com/<USERNAME>` and
   `runnerScaleSetName: ghost-runners-personal`.
4. Maintainer uses `runs-on: ghost-runners-personal` in their personal repo.

See `/docs/ops/maintainer-onboarding.md` for the full Application manifest and
security notes.

**ArgoCD Applications for ARC** (stored in `argocd/`, applied manually once):
- `arc-systems` — controller (gha-runner-scale-set-controller 0.9.3)
- `arc-runners` — scale set pointing at `https://github.com/projectbluefin`
- `arc-runners-personal` (optional) — second scale set for a maintainer's
  personal GitHub account; uses a different `githubConfigUrl` and installation
  secret.

**GitHub App:** `bluefin-ghost-arc` (App ID 4099840, Installation 141458121)
installed on the `projectbluefin` org. Credentials in `arc-github-secret`
(namespace `arc-runners`) — never replace with a PAT. Personal-account scale
sets need a separate secret with the personal installation ID.

---
