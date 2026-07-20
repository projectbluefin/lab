# Bootc Image Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand lab image ingest so requested streams (including Fedora bootc and ublue main) are polled and trigger full-matrix QA on digest change.

**Architecture:** Reuse the existing `image-poller` WorkflowTemplate and add explicit CronWorkflows per image stream. Keep one-stream-per-cron isolation with `concurrencyPolicy: Forbid`, and store per-stream digests in `image-polling-digests` ConfigMap keys. Roll out through GitOps by editing `manifests/` and `argo/workflow-templates/`.

**Tech Stack:** Argo Workflows CronWorkflow/WorkflowTemplate (`argoproj.io/v1alpha1`), Kubernetes ConfigMap, skopeo digest inspection, `just lint`.

## Global Constraints

- Keep GitOps ownership: edit tracked YAML only; no `kubectl apply` for `manifests/` or `argo/workflow-templates/`.
- Keep poll overlap protection: `concurrencyPolicy: Forbid` on every poll CronWorkflow.
- Use 3-hour cadence for mass coverage, with staggered minute offsets per stream.
- Full matrix trigger on digest change: `suites=smoke,common,developer,software,system`.
- Keep image pollers isolated by stream (no global auto-discovery engine in this wave).
- Use allowed registries only (repo lint policy).
- Validate YAML changes with `just lint` before merge.

---

### Task 1: Make image-poller defaults match full-matrix policy

**Files:**
- Modify: `argo/workflow-templates/image-poller.yaml`

**Interfaces:**
- Consumes: Existing poll CronWorkflows passing `image`, `image-tag`, `state-key`, `namespace`, `variant`, optional `containerdisk-tag`.
- Produces: Updated default matrix behavior for all pollers that do not override `suites`.

- [ ] **Step 1: Update default suites parameter to full matrix**

```yaml
# argo/workflow-templates/image-poller.yaml
spec:
  arguments:
    parameters:
      - name: suites
        value: "smoke,common,developer,software,system"
```

- [ ] **Step 2: Ensure run-pipeline uses vm-memory parameter (not hardcoded literal)**

```yaml
# argo/workflow-templates/image-poller.yaml
      - name: vm-memory
        value: "8Gi"
...
                - name: vm-memory
                  value: "{{workflow.parameters.vm-memory}}"
```

- [ ] **Step 3: Keep explicit failure behavior in digest fetch/update**

```yaml
# Keep this behavior unchanged:
# - digest fetch failures emit "none"
# - update step writes changed=false and exits 0 on "none"
# - real changes patch image-polling-digests
```

- [ ] **Step 4: Run lint**

Run: `cd /var/home/jorge/src/lab && just lint`  
Expected: success (no YAML or policy errors).

- [ ] **Step 5: Commit**

```bash
git add argo/workflow-templates/image-poller.yaml
git commit -m "feat(poller): default digest-triggered runs to full matrix"
```

### Task 2: Expand digest state keys for all new streams

**Files:**
- Modify: `manifests/image-polling-state.yaml`

**Interfaces:**
- Consumes: Existing `image-poller` `state-key` lookup (`image-polling-digests` ConfigMap).
- Produces: New keys for latest/main/Fedora streams so first run initializes cleanly.

- [ ] **Step 1: Add missing state keys**

```yaml
# manifests/image-polling-state.yaml
data:
  digest-bluefin-stable: ""
  digest-bluefin-testing: ""
  digest-bluefin-main: ""
  digest-lts-stable: ""
  digest-lts-testing: ""
  digest-lts-latest: ""
  digest-aurora-stable: ""
  digest-aurora-testing: ""
  digest-aurora-main: ""
  digest-bazzite-stable: ""
  digest-bazzite-testing: ""
  digest-bazzite-main: ""
  digest-kinoite-43: ""
  digest-kinoite-44: ""
  digest-akmods-main-43: ""
  digest-akmods-main-44: ""
  digest-dakota-latest: ""
  digest-fedora-bootc-stable: ""
  digest-fedora-bootc-testing: ""
```

- [ ] **Step 2: Keep ConfigMap name and namespace unchanged**

```yaml
metadata:
  name: image-polling-digests
  namespace: argo
```

- [ ] **Step 3: Run lint**

Run: `cd /var/home/jorge/src/lab && just lint`  
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add manifests/image-polling-state.yaml
git commit -m "feat(poller): add digest state keys for latest/main/fedora streams"
```

### Task 3: Normalize existing pollers to full-matrix suites and add missing latest stream

**Files:**
- Modify:
  - `manifests/image-poll-bluefin-testing.yaml`
  - `manifests/image-poll-bluefin-stable.yaml`
  - `manifests/image-poll-lts-testing.yaml`
  - `manifests/image-poll-lts-stable.yaml`
  - `manifests/image-poll-aurora-testing.yaml`
  - `manifests/image-poll-aurora-stable.yaml`
  - `manifests/image-poll-bazzite-testing.yaml`
  - `manifests/image-poll-bazzite-stable.yaml`
- Create:
  - `manifests/image-poll-lts-latest.yaml`

**Interfaces:**
- Consumes: `image-poller` WorkflowTemplate arguments.
- Produces: Existing stream pollers all trigger full matrix on digest change.

- [ ] **Step 1: Update suites on existing pollers**

```yaml
# For every poller above that uses workflowTemplateRef: image-poller
arguments:
  parameters:
    - name: suites
      value: "smoke,common,developer,software,system"
```

- [ ] **Step 2: Add lts latest poller**

```yaml
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: image-poll-lts-latest
  namespace: argo
spec:
  schedules:
    - "55 */3 * * *"
  timezone: "UTC"
  concurrencyPolicy: Forbid
  startingDeadlineSeconds: 300
  workflowSpec:
    serviceAccountName: argo
    workflowTemplateRef:
      name: image-poller
    arguments:
      parameters:
        - name: image
          value: "ghcr.io/projectbluefin/bluefin-lts"
        - name: image-tag
          value: "latest"
        - name: state-key
          value: "digest-lts-latest"
        - name: namespace
          value: "bluefin-lts-test"
        - name: suites
          value: "smoke,common,developer,software,system"
        - name: vm-memory
          value: "8Gi"
        - name: variant
          value: "bluefin"
        - name: containerdisk-tag
          value: "lts-latest"
```

- [ ] **Step 3: Run lint**

Run: `cd /var/home/jorge/src/lab && just lint`  
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add manifests/image-poll-bluefin-testing.yaml manifests/image-poll-bluefin-stable.yaml \
  manifests/image-poll-lts-testing.yaml manifests/image-poll-lts-stable.yaml \
  manifests/image-poll-aurora-testing.yaml manifests/image-poll-aurora-stable.yaml \
  manifests/image-poll-bazzite-testing.yaml manifests/image-poll-bazzite-stable.yaml \
  manifests/image-poll-lts-latest.yaml
git commit -m "feat(poller): run full matrix on digest change for existing image streams"
```

### Task 4: Add Fedora bootc and ublue main pollers

**Files:**
- Create:
  - `manifests/image-poll-fedora-bootc-stable.yaml`
  - `manifests/image-poll-fedora-bootc-testing.yaml`
  - `manifests/image-poll-bluefin-main.yaml`
  - `manifests/image-poll-aurora-main.yaml`
  - `manifests/image-poll-bazzite-main.yaml`

**Interfaces:**
- Consumes: `image-poller` WorkflowTemplate.
- Produces: New stream coverage for Fedora bootc + requested ublue main channels.

- [ ] **Step 1: Add Fedora bootc stable/testing pollers**

```yaml
# manifests/image-poll-fedora-bootc-stable.yaml
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: image-poll-fedora-bootc-stable
  namespace: argo
spec:
  schedules: ["10 */3 * * *"]
  timezone: "UTC"
  concurrencyPolicy: Forbid
  workflowSpec:
    serviceAccountName: argo
    workflowTemplateRef: { name: image-poller }
    arguments:
      parameters:
        - { name: image, value: "quay.io/fedora/fedora-bootc" }
        - { name: image-tag, value: "stable" }
        - { name: state-key, value: "digest-fedora-bootc-stable" }
        - { name: namespace, value: "bluefin-test" }
        - { name: suites, value: "smoke,common,developer,software,system" }
        - { name: variant, value: "bluefin" }
        - { name: containerdisk-tag, value: "fedora-bootc-stable" }

# manifests/image-poll-fedora-bootc-testing.yaml
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: image-poll-fedora-bootc-testing
  namespace: argo
spec:
  schedules: ["40 */3 * * *"]
  timezone: "UTC"
  concurrencyPolicy: Forbid
  workflowSpec:
    serviceAccountName: argo
    workflowTemplateRef: { name: image-poller }
    arguments:
      parameters:
        - { name: image, value: "quay.io/fedora/fedora-bootc" }
        - { name: image-tag, value: "testing" }
        - { name: state-key, value: "digest-fedora-bootc-testing" }
        - { name: namespace, value: "bluefin-test" }
        - { name: suites, value: "smoke,common,developer,software,system" }
        - { name: variant, value: "bluefin" }
        - { name: containerdisk-tag, value: "fedora-bootc-testing" }
```

- [ ] **Step 2: Add ublue main pollers for bluefin/aurora/bazzite**

```yaml
# manifests/image-poll-bluefin-main.yaml
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: image-poll-bluefin-main
  namespace: argo
spec:
  schedules: ["12 */3 * * *"]
  timezone: "UTC"
  concurrencyPolicy: Forbid
  workflowSpec:
    serviceAccountName: argo
    workflowTemplateRef: { name: image-poller }
    arguments:
      parameters:
        - { name: image, value: "ghcr.io/ublue-os/bluefin" }
        - { name: image-tag, value: "main" }
        - { name: state-key, value: "digest-bluefin-main" }
        - { name: namespace, value: "bluefin-test" }
        - { name: suites, value: "smoke,common,developer,software,system" }
        - { name: variant, value: "bluefin" }
        - { name: containerdisk-tag, value: "bluefin-main" }

# manifests/image-poll-aurora-main.yaml
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: image-poll-aurora-main
  namespace: argo
spec:
  schedules: ["22 */3 * * *"]
  timezone: "UTC"
  concurrencyPolicy: Forbid
  workflowSpec:
    serviceAccountName: argo
    workflowTemplateRef: { name: image-poller }
    arguments:
      parameters:
        - { name: image, value: "ghcr.io/ublue-os/aurora" }
        - { name: image-tag, value: "main" }
        - { name: state-key, value: "digest-aurora-main" }
        - { name: namespace, value: "aurora-test" }
        - { name: suites, value: "smoke,common,developer,software,system" }
        - { name: variant, value: "aurora" }
        - { name: containerdisk-tag, value: "aurora-main" }

# manifests/image-poll-bazzite-main.yaml
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: image-poll-bazzite-main
  namespace: argo
spec:
  schedules: ["32 */3 * * *"]
  timezone: "UTC"
  concurrencyPolicy: Forbid
  workflowSpec:
    serviceAccountName: argo
    workflowTemplateRef: { name: image-poller }
    arguments:
      parameters:
        - { name: image, value: "ghcr.io/ublue-os/bazzite" }
        - { name: image-tag, value: "main" }
        - { name: state-key, value: "digest-bazzite-main" }
        - { name: namespace, value: "bazzite-test" }
        - { name: suites, value: "smoke,common,developer,software,system" }
        - { name: variant, value: "bazzite" }
        - { name: containerdisk-tag, value: "bazzite-main" }
```

- [ ] **Step 3: Run lint**

Run: `cd /var/home/jorge/src/lab && just lint`  
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add manifests/image-poll-fedora-bootc-stable.yaml manifests/image-poll-fedora-bootc-testing.yaml \
  manifests/image-poll-bluefin-main.yaml manifests/image-poll-aurora-main.yaml manifests/image-poll-bazzite-main.yaml
git commit -m "feat(poller): add fedora bootc and ublue main digest pollers"
```

### Task 5: Rollout verification and evidence capture

**Files:**
- Modify (if needed for docs): `/docs/reference/agent-cheatsheet.md` (image poller section)

**Interfaces:**
- Consumes: All manifests/templates from Tasks 1-4.
- Produces: Verified running pollers and observed full-matrix trigger behavior.

- [ ] **Step 1: Verify ArgoCD reconciliation and CronWorkflow presence**

Run:

```bash
export KUBECONFIG=~/.kube/bluespeed.yaml
kubectl get application lab-infra -n argocd -o jsonpath='{.status.sync.status} {.status.health.status}{"\n"}'
argo cron list -n argo | rg 'image-poll-(bluefin|lts|aurora|bazzite|kinoite|fedora-bootc|dakota|akmods)'
```

Expected:
- App status `Synced Healthy`
- New cron names present

- [ ] **Step 2: Trigger representative new pollers and confirm behavior**

Run:

```bash
argo submit -n argo --from cronworkflow/image-poll-fedora-bootc-testing
argo submit -n argo --from cronworkflow/image-poll-bluefin-main
argo submit -n argo --from cronworkflow/image-poll-aurora-main
```

Expected:
- Poll workflows complete
- If digest changed, QA workflow appears with matrix suites; if unchanged, no QA workflow spawned

- [ ] **Step 3: Verify state-key mutation**

Run:

```bash
kubectl get configmap image-polling-digests -n argo -o yaml | rg 'digest-(fedora-bootc|bluefin-main|aurora-main|bazzite-main|lts-latest)'
```

Expected: keys exist; changed streams have non-empty digest values.

- [ ] **Step 4: Final commit for any doc touch-ups**

```bash
git add /docs/reference/agent-cheatsheet.md
git commit -m "docs: add expanded image poller operations notes"
```
