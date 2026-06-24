# Dakota Cluster Build Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run Dakota BST builds as parallel Argo Workflow pods on the homelab k8s cluster, sharing an in-cluster buildbox-casd CAS on ghost's NVMe, triggered by push to `dakota:testing`.

**Architecture:** buildbox-casd runs as a k8s Deployment in `local-registry` namespace backed by a 200Gi PVC on ghost. Two parallel Argo WorkflowTemplate steps (build-bluefin, build-bluefin-nvidia) each run the bst2 container pointing at that in-cluster CAS. Push to `dakota:testing` → GHA cluster-build.yml → Argo submit. Resource limits + `bst-build` PriorityClass ensure builds never starve test VMs.

**Tech Stack:** Argo Workflows v3.6+, buildbox-casd (from bst2 image), BST 2.5+, k3s local-path PVC, Zot (192.168.1.102:30500), GitHub API (commit polling, no ARC runners)

## Global Constraints

- All `image:` refs in `argo/` and `manifests/` must use allowed registries: `ghcr.io`, `quay.io`, `registry.fedoraproject.org`, `registry.access.redhat.com`, `registry.k8s.io`, `cgr.dev`, `192.168.1.102`, `localhost`. Mirror any other image to `192.168.1.102:30500` first.
- bst2 image SHA (pin exactly): `registry.gitlab.com/freedesktop-sdk/infrastructure/freedesktop-sdk-docker-images/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d` → mirrored to `192.168.1.102:30500/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d`
- Never `kubectl apply` WorkflowTemplates — ArgoCD owns reconciliation. Push to main and let ArgoCD sync.
- Argo v3.6+: use `synchronization.semaphores` (list), `schedules:` (array).
- VM names ≤63 chars.
- BST build pods: `requests.memory=16Gi` — this naturally excludes exo-1 (15.1Gi allocatable). No nodeSelector needed.
- Run `just lint` after every change to testing-lab `argo/` or `manifests/`.
- No SSH to ghost. No kubectl apply for ArgoCD-managed resources.

## Cluster Nodes (current)

| Node     | CPUs | RAM    | BST eligible | Notes                    |
|----------|------|--------|--------------|--------------------------|
| ghost    | 32   | 62.5Gi | yes          | casd PVC lives here      |
| bazzite  | 12   | 30.5Gi | yes          | overflow build pods      |
| bluefin  | 16   | 31.2Gi | yes          | hamilton workstation     |
| exo-1    | 22   | 15.1Gi | no           | excluded by 16Gi request |

---

### Task 1: Mirror bst2 image to local Zot

**Files:**
- No repo changes — one-time operational step on the workstation

**Interfaces:**
- Produces: `192.168.1.102:30500/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d` (used by Tasks 3 and 5)

- [ ] **Step 1: Mirror bst2 image**

```bash
skopeo copy \
  docker://registry.gitlab.com/freedesktop-sdk/infrastructure/freedesktop-sdk-docker-images/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d \
  docker://192.168.1.102:30500/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d \
  --dest-tls-verify=false
```

Expected: `Getting image source signatures` → `Copying blob` → `Writing manifest to image destination`

- [ ] **Step 2: Verify image in Zot catalog**

```bash
curl -s http://192.168.1.102:30500/v2/_catalog | python3 -m json.tool
```

Expected: `"repositories"` list includes `"bst2"`.

---

### Task 2: PriorityClass for BST builds

**Files:**
- Create: `manifests/bst-build-priorityclass.yaml`

**Interfaces:**
- Produces: `priorityClassName: bst-build` (value=500000) used in Task 5 WorkflowTemplate

- [ ] **Step 1: Create the PriorityClass manifest**

```yaml
# manifests/bst-build-priorityclass.yaml
# BST build pods — below lab-test-vm (1,000,000) so test VMs always win
# scheduling disputes. PreemptionPolicy: Never means BST won't evict others,
# but can itself be preempted. Since buildbox-casd preserves all artifacts,
# a preempted build restarts and skips already-built elements.
apiVersion: scheduling.k8s.io/v1
kind: PriorityClass
metadata:
  name: bst-build
  labels:
    app.kubernetes.io/part-of: testing-lab
value: 500000
preemptionPolicy: Never
globalDefault: false
description: "BST build pods. Lower priority than lab-test-vm. Preemptable on resource contention."
```

- [ ] **Step 2: Lint**

```bash
cd /var/home/jorge/src/testing-lab && just lint
```

Expected: exit 0, no errors.

- [ ] **Step 3: Commit and push**

```bash
git add manifests/bst-build-priorityclass.yaml
git commit -m "feat: add bst-build PriorityClass for dakota cluster builds"
git push
```

- [ ] **Step 4: Verify ArgoCD sync**

```bash
kubectl get priorityclass bst-build
```

Expected: `NAME: bst-build  VALUE: 500000` (within ~3 min of push).

---

### Task 3: buildbox-casd Deployment

**Files:**
- Create: `manifests/buildbox-casd.yaml`

**Interfaces:**
- Produces: gRPC endpoint `buildbox-casd.local-registry.svc.cluster.local:11002` (used by Task 5 BST config)
- Consumes: `192.168.1.102:30500/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d` from Task 1

- [ ] **Step 1: Create the manifest**

```yaml
# manifests/buildbox-casd.yaml
# In-cluster CAS for dakota BST builds. Stores artifacts on ghost NVMe (local-path PVC).
# Build pods in any namespace reach it at buildbox-casd.local-registry.svc.cluster.local:11002.
# Pinned to ghost via nodeSelector — local-path PVC follows the pod's node.
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: buildbox-casd-data
  namespace: local-registry
  labels:
    app.kubernetes.io/component: buildbox-casd
    app.kubernetes.io/part-of: testing-lab
spec:
  storageClassName: local-path
  accessModes: [ReadWriteOnce]
  resources:
    requests:
      storage: 200Gi
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: buildbox-casd
  namespace: local-registry
  labels:
    app.kubernetes.io/component: buildbox-casd
    app.kubernetes.io/part-of: testing-lab
spec:
  replicas: 1
  selector:
    matchLabels:
      app: buildbox-casd
  template:
    metadata:
      labels:
        app: buildbox-casd
        app.kubernetes.io/component: buildbox-casd
        app.kubernetes.io/part-of: testing-lab
    spec:
      nodeSelector:
        kubernetes.io/hostname: ghost
      containers:
        - name: casd
          image: 192.168.1.102:30500/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d
          command:
            - buildbox-casd
            - --bind
            - 0.0.0.0:11002
            - --quota-high
            - 190G
            - /data
          ports:
            - name: grpc
              containerPort: 11002
              protocol: TCP
          volumeMounts:
            - name: data
              mountPath: /data
          resources:
            requests:
              cpu: 200m
              memory: 256Mi
            limits:
              cpu: "2"
              memory: 1Gi
          readinessProbe:
            tcpSocket:
              port: 11002
            initialDelaySeconds: 5
            periodSeconds: 10
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: buildbox-casd-data
---
apiVersion: v1
kind: Service
metadata:
  name: buildbox-casd
  namespace: local-registry
  labels:
    app.kubernetes.io/component: buildbox-casd
    app.kubernetes.io/part-of: testing-lab
spec:
  selector:
    app: buildbox-casd
  ports:
    - name: grpc
      port: 11002
      targetPort: 11002
      protocol: TCP
```

- [ ] **Step 2: Lint**

```bash
cd /var/home/jorge/src/testing-lab && just lint
```

Expected: exit 0.

- [ ] **Step 3: Commit and push**

```bash
git add manifests/buildbox-casd.yaml
git commit -m "feat: add buildbox-casd Deployment for dakota cluster builds"
git push
```

- [ ] **Step 4: Wait for ArgoCD sync and verify pod**

```bash
kubectl get pod -n local-registry -l app=buildbox-casd
kubectl get pvc -n local-registry buildbox-casd-data
```

Expected: pod `Running` with `1/1` ready. PVC `Bound`.

If pod is in `ImagePullBackOff`, the bst2 mirror from Task 1 did not complete — re-run Task 1 Step 1.

- [ ] **Step 5: Smoke-test casd is listening**

```bash
kubectl run casd-probe --rm -i --restart=Never \
  --image=cgr.dev/chainguard/kubectl:latest-dev \
  -n argo -- \
  bash -c 'curl -sf --max-time 5 http://buildbox-casd.local-registry.svc.cluster.local:11002 || echo "gRPC port open (non-HTTP response is normal)"'
```

Expected: connection accepted (even if response is a gRPC error — that's normal for a raw curl to a gRPC port).

---

### Task 4: Semaphore updates

**Files:**
- Modify: `manifests/semaphore-config.yaml`
- Modify: `manifests/semaphore-tuner.yaml`

**Interfaces:**
- Produces: `semaphore-config` key `max-bst-builds: "1"` (consumed by Task 5 WorkflowTemplate)
- Produces: semaphore-tuner reserves 16Gi on ghost for BST builds

- [ ] **Step 1: Add max-bst-builds to semaphore-config**

In `manifests/semaphore-config.yaml`, add one key to the `data:` section:

```yaml
  # BST build pods — one at a time initially. Bump to 2 when more nodes arrive.
  max-bst-builds: "1"
```

The full `data:` block becomes:

```yaml
data:
  # containerdisk VMs (bluefin, dakota) — any Ready node
  # hostdisk VMs (knuckle, flatcar, gnomeos) — ghost only
  # Recomputed hourly by the semaphore-tuner CronWorkflow.
  # These bootstrap values are overwritten on the first tuner run.
  max-containerdisk-vms: "8"
  max-hostdisk-vms: "3"
  # BST build pods — one at a time initially. Bump to 2 when more nodes arrive.
  max-bst-builds: "1"
```

- [ ] **Step 2: Update semaphore-tuner to reserve BST headroom on ghost**

In `manifests/semaphore-tuner.yaml`, update the tunables block and the ghost calculation.

Change:
```bash
            # ── tunables ────────────────────────────────────────────────────
            OVERHEAD_GI=12   # system/argo overhead reserved per node (right-sized for desktops with active workloads)
            SLOT_GI=8        # slot unit = largest VM in the fleet
            MIN_CD=2;  MAX_CD=16   # containerdisk bounds (any node)
            MIN_HD=2;  MAX_HD=6    # hostdisk bounds (ghost only)
```

To:
```bash
            # ── tunables ────────────────────────────────────────────────────
            OVERHEAD_GI=12   # system/argo overhead reserved per node (right-sized for desktops with active workloads)
            SLOT_GI=8        # slot unit = largest VM in the fleet
            MIN_CD=2;  MAX_CD=16   # containerdisk bounds (any node)
            MIN_HD=2;  MAX_HD=6    # hostdisk bounds (ghost only)
            BST_RESERVE_GI=16  # always reserve headroom on ghost for one BST build pod
```

And change the per-node loop body. Find:
```bash
              mem_gi=$(mem_to_gi "$mem_raw")
              usable=$(( mem_gi > OVERHEAD_GI ? mem_gi - OVERHEAD_GI : 0 ))
              total_cd_gi=$(( total_cd_gi + usable ))
              echo "  $name: ${mem_gi}Gi allocatable → ${usable}Gi usable"
              [[ "$name" == "ghost" ]] && ghost_gi=$usable
```

Replace with:
```bash
              mem_gi=$(mem_to_gi "$mem_raw")
              usable=$(( mem_gi > OVERHEAD_GI ? mem_gi - OVERHEAD_GI : 0 ))
              # Reserve BST build headroom on ghost so build pods always fit
              [[ "$name" == "ghost" ]] && usable=$(( usable > BST_RESERVE_GI ? usable - BST_RESERVE_GI : 0 ))
              total_cd_gi=$(( total_cd_gi + usable ))
              echo "  $name: ${mem_gi}Gi allocatable → ${usable}Gi usable"
              [[ "$name" == "ghost" ]] && ghost_gi=$usable
```

Result: ghost contributes `floor((62 - 12 - 16) / 8) = 4` VM slots instead of 6. Total with bazzite(2)+bluefin(2)+exo-1(0)+ghost(4) = 8 slots — same as before, just distributed differently.

- [ ] **Step 3: Lint**

```bash
cd /var/home/jorge/src/testing-lab && just lint
```

Expected: exit 0.

- [ ] **Step 4: Commit and push**

```bash
git add manifests/semaphore-config.yaml manifests/semaphore-tuner.yaml
git commit -m "feat: add max-bst-builds semaphore and BST_RESERVE_GI to tuner"
git push
```

- [ ] **Step 5: Verify semaphore-config has the new key**

```bash
kubectl get configmap semaphore-config -n argo -o jsonpath='{.data}' | python3 -m json.tool
```

Expected: `"max-bst-builds": "1"` present alongside the vm keys.

---

### Task 5: Argo WorkflowTemplate — dakota-build-pipeline

**Files:**
- Create: `argo/workflow-templates/dakota-build-pipeline.yaml`

**Interfaces:**
- Consumes: `semaphore-config.max-bst-builds` (Task 4)
- Consumes: `192.168.1.102:30500/bst2:...` (Task 1)
- Consumes: `buildbox-casd.local-registry.svc.cluster.local:11002` (Task 3)
- Consumes: `priorityClassName: bst-build` (Task 2)
- Produces: WorkflowTemplate `dakota-build-pipeline` in `argo` namespace (consumed by Task 8 GHA trigger)

- [ ] **Step 1: Create the WorkflowTemplate**

```yaml
# argo/workflow-templates/dakota-build-pipeline.yaml
apiVersion: argoproj.io/v1alpha1
kind: WorkflowTemplate
metadata:
  name: dakota-build-pipeline
  namespace: argo
  annotations:
    description: |
      Parallel dakota BST build pipeline. Builds bluefin and bluefin-nvidia OCI variants
      in parallel against the in-cluster buildbox-casd CAS on ghost. Triggered by push to
      dakota:testing via GHA cluster-build.yml. On success, exports to local Zot (:30500).
      Uses bst-build PriorityClass — preemptable by lab-test-vm pods.
  labels:
    app.kubernetes.io/component: dakota-build
    app.kubernetes.io/part-of: bluefin-test-suite
spec:
  serviceAccountName: argo
  synchronization:
    semaphores:
      - configMapKeyRef:
          name: semaphore-config
          key: max-bst-builds
  activeDeadlineSeconds: 7200

  arguments:
    parameters:
      - name: repo
        value: "https://github.com/projectbluefin/dakota.git"
      - name: ref
        value: "testing"

  templates:

    - name: build
      dag:
        tasks:
          - name: build-bluefin
            template: bst-build
            arguments:
              parameters:
                - name: element
                  value: "oci/bluefin.bst"
                - name: tag
                  value: "dakota-cluster-testing"
          - name: build-bluefin-nvidia
            template: bst-build
            arguments:
              parameters:
                - name: element
                  value: "oci/bluefin-nvidia.bst"
                - name: tag
                  value: "dakota-cluster-testing-nvidia"

    - name: bst-build
      inputs:
        parameters:
          - name: element
          - name: tag
      podSpecPatch: |
        containers:
          - name: main
            securityContext:
              privileged: true
              seLinuxOptions:
                type: spc_t
      volumes:
        - name: fuse
          hostPath:
            path: /dev/fuse
        - name: bst-cache
          emptyDir: {}
      script:
        image: 192.168.1.102:30500/bst2:64eb0b4930d57a92710822898fb73af6cc1ae35d
        command: [bash]
        resources:
          requests:
            cpu: "4"
            memory: 16Gi
          limits:
            cpu: "8"
            memory: 28Gi
        priorityClassName: bst-build
        volumeMounts:
          - name: fuse
            mountPath: /dev/fuse
          - name: bst-cache
            mountPath: /root/.cache/buildstream
        source: |
          set -euo pipefail

          # Write in-cluster BST config — plain gRPC to in-cluster casd, no TLS
          cat > /tmp/buildstream-cluster.conf << 'BSTCONF'
          cache:
            storage-service:
              url: http://buildbox-casd.local-registry.svc.cluster.local:11002
              push: true
          BSTCONF

          BST_CONF=/tmp/buildstream-cluster.conf
          ELEMENT="{{inputs.parameters.element}}"
          TAG="{{inputs.parameters.tag}}"
          REF="{{workflow.parameters.ref}}"
          REPO="{{workflow.parameters.repo}}"

          echo "=== Cloning dakota @ ${REF} ==="
          git clone --depth=1 --branch "${REF}" "${REPO}" /src
          cd /src

          echo "=== Building ${ELEMENT} ==="
          bst --config "${BST_CONF}" \
              --no-interactive \
              -o x86_64_v3 true \
              build "${ELEMENT}"

          echo "=== Exporting ${ELEMENT} ==="
          EXPORT_DIR="/tmp/export-$(basename ${ELEMENT} .bst)"
          mkdir -p "${EXPORT_DIR}"
          bst --config "${BST_CONF}" \
              --no-interactive \
              artifact checkout "${ELEMENT}" \
              --directory "${EXPORT_DIR}"

          echo "=== Pushing to local Zot ==="
          skopeo copy \
            --dest-tls-verify=false \
            "oci:${EXPORT_DIR}" \
            "docker://192.168.1.102:30500/${TAG}:latest"

          echo "=== Build complete: 192.168.1.102:30500/${TAG}:latest ==="

  entrypoint: build
```

- [ ] **Step 2: Lint**

```bash
cd /var/home/jorge/src/testing-lab && just lint
```

Expected: exit 0.

- [ ] **Step 3: Commit and push**

```bash
git add argo/workflow-templates/dakota-build-pipeline.yaml
git commit -m "feat: add dakota-build-pipeline WorkflowTemplate"
git push
```

- [ ] **Step 4: Verify ArgoCD sync**

```bash
kubectl get workflowtemplate dakota-build-pipeline -n argo
```

Expected: resource exists (within ~3 min of push).

- [ ] **Step 5: Manual test — submit the workflow**

```bash
argo submit --watch -n argo \
  --from workflowtemplate/dakota-build-pipeline \
  -p ref=testing
```

Watch for the DAG to fan out two pods. Expected: both pods transition `Running → Succeeded`.

If pods crash with `Error: sandbox setup failed`, the `privileged: true` + `spc_t` combination is not working. Check with:
```bash
kubectl logs -n argo <pod-name> -c main | tail -30
```

---

### Task 6: Commit-poller trigger (replaces ARC/GHA)

No ARC runners. Trigger is an in-cluster CronWorkflow that polls GitHub API for new commits on `dakota:testing` — same pattern as image-poller. State stored in `image-polling-digests` ConfigMap (already exists).

**Files:**
- Modify: `manifests/image-polling-state.yaml` (seed `sha-dakota-testing` key)
- Create: `argo/workflow-templates/dakota-commit-poller.yaml`
- Create: `manifests/dakota-commit-poller.yaml`
- Create: `buildstream-cluster.conf` in dakota repo (used by build pods, not the trigger)

**Interfaces:**
- Consumes: `github-token` Secret in `argo` namespace (already exists)
- Consumes: `image-polling-digests` ConfigMap in `argo` namespace (already exists)
- Consumes: WorkflowTemplate `dakota-build-pipeline` from Task 5
- Produces: automated cluster build whenever `dakota:testing` HEAD SHA changes

- [ ] **Step 1: Seed the state key in image-polling-state.yaml**

In `manifests/image-polling-state.yaml`, change `data: {}` to:

```yaml
data:
  sha-dakota-testing: ""
```

- [ ] **Step 2: Create the WorkflowTemplate**

```yaml
# argo/workflow-templates/dakota-commit-poller.yaml
apiVersion: argoproj.io/v1alpha1
kind: WorkflowTemplate
metadata:
  name: dakota-commit-poller
  namespace: argo
  annotations:
    description: |
      Polls GitHub API for new commits on dakota:testing. If HEAD SHA changed
      vs stored state in image-polling-digests ConfigMap, submits
      dakota-build-pipeline. Runs every 5 min via the dakota-commit-poller
      CronWorkflow in manifests/.
  labels:
    app.kubernetes.io/component: commit-poller
    app.kubernetes.io/part-of: bluefin-test-suite
spec:
  serviceAccountName: argo
  podGC:
    strategy: OnWorkflowCompletion
  ttlStrategy:
    secondsAfterSuccess: 3600
    secondsAfterFailure: 86400
  activeDeadlineSeconds: 120
  entrypoint: poll

  arguments:
    parameters:
      - name: repo
        value: "projectbluefin/dakota"
      - name: branch
        value: "testing"
      - name: state-key
        value: "sha-dakota-testing"

  templates:

    - name: poll
      dag:
        tasks:
          - name: check-sha
            template: check-sha
          - name: run-build
            depends: "check-sha.Succeeded"
            when: "{{tasks.check-sha.outputs.parameters.changed}} == true"
            templateRef:
              name: dakota-build-pipeline
              template: build
            arguments:
              parameters:
                - name: ref
                  value: "{{workflow.parameters.branch}}"

    - name: check-sha
      outputs:
        parameters:
          - name: changed
            valueFrom:
              path: /tmp/changed
      script:
        image: cgr.dev/chainguard/kubectl:latest-dev
        command: [bash]
        resources:
          requests:
            cpu: 100m
            memory: 64Mi
          limits:
            cpu: 200m
            memory: 128Mi
        env:
          - name: GITHUB_TOKEN
            valueFrom:
              secretKeyRef:
                name: github-token
                key: token
        source: |
          set -euo pipefail
          REPO="{{workflow.parameters.repo}}"
          BRANCH="{{workflow.parameters.branch}}"
          KEY="{{workflow.parameters.state-key}}"

          # Fetch latest commit SHA from GitHub API
          REMOTE=$(curl -sf \
            -H "Authorization: Bearer ${GITHUB_TOKEN}" \
            -H "Accept: application/vnd.github+json" \
            "https://api.github.com/repos/${REPO}/commits?sha=${BRANCH}&per_page=1" \
            | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['sha'])")

          echo "Remote SHA:  ${REMOTE}" >&2

          # Read stored SHA from ConfigMap
          STORED=$(kubectl get configmap image-polling-digests \
            -n argo \
            -o jsonpath="{.data.${KEY}}" 2>/dev/null || true)

          echo "Stored SHA:  ${STORED}" >&2

          if [[ "$REMOTE" == "$STORED" ]]; then
            echo "No change — skipping build" >&2
            echo "false" > /tmp/changed
          else
            echo "SHA changed — updating ConfigMap key ${KEY}" >&2
            kubectl patch configmap image-polling-digests \
              -n argo --type merge \
              -p "{\"data\":{\"${KEY}\":\"${REMOTE}\"}}" 2>/dev/null \
            || kubectl patch configmap image-polling-digests \
              -n argo --type merge \
              -p "{\"data\":{\"${KEY}\":\"${REMOTE}\"}}"
            echo "true" > /tmp/changed
          fi
```

- [ ] **Step 3: Create the CronWorkflow trigger**

```yaml
# manifests/dakota-commit-poller.yaml
# Polls dakota:testing for new commits every 5 min. Triggers in-cluster BST build
# when HEAD SHA changes. No ARC runners — fully in-cluster trigger.
apiVersion: argoproj.io/v1alpha1
kind: CronWorkflow
metadata:
  name: dakota-commit-poller
  namespace: argo
  labels:
    app.kubernetes.io/component: commit-poller
    app.kubernetes.io/part-of: bluefin-test-suite
spec:
  schedules:
    - "*/5 * * * *"
  timezone: "UTC"
  concurrencyPolicy: Forbid
  startingDeadlineSeconds: 60
  workflowSpec:
    serviceAccountName: argo
    podGC:
      strategy: OnWorkflowCompletion
    ttlStrategy:
      secondsAfterSuccess: 3600
      secondsAfterFailure: 86400
    workflowTemplateRef:
      name: dakota-commit-poller
```

- [ ] **Step 4: Create buildstream-cluster.conf in the dakota repo**

```yaml
# buildstream-cluster.conf
# BST user config for in-cluster builds. Points at the in-cluster buildbox-casd
# on ghost — plain gRPC, no TLS (cluster-internal only).
# Usage: bst --config buildstream-cluster.conf build oci/bluefin.bst
cache:
  storage-service:
    url: http://buildbox-casd.local-registry.svc.cluster.local:11002
    push: true
```

Save to `/var/home/jorge/src/dakota/buildstream-cluster.conf` and commit:

```bash
cd /var/home/jorge/src/dakota
git add buildstream-cluster.conf
git commit -m "feat: add buildstream-cluster.conf for homelab cluster builds"
git push origin testing
```

- [ ] **Step 5: Lint, commit, push testing-lab changes**

```bash
cd /var/home/jorge/src/testing-lab && just lint
git add \
  manifests/image-polling-state.yaml \
  argo/workflow-templates/dakota-commit-poller.yaml \
  manifests/dakota-commit-poller.yaml
git commit -m "feat: add dakota commit-poller trigger for cluster builds"
git push
```

Expected: lint exit 0.

- [ ] **Step 6: Verify ArgoCD sync and CronWorkflow**

```bash
kubectl get cronworkflow dakota-commit-poller -n argo
kubectl get workflowtemplate dakota-commit-poller -n argo
```

Expected: both resources exist.

- [ ] **Step 7: Manual smoke-test of the poller**

```bash
# Force a poll run immediately
argo submit -n argo --from cronworkflow/dakota-commit-poller --watch
```

Expected: `check-sha` step runs. If dakota:testing HEAD ≠ stored SHA → `run-build` step fires. If already up to date → workflow succeeds with `changed=false` (run-build skipped).

---

### Task 8: Update design spec + AGENTS.md node table

**Files:**
- Modify: `docs/superpowers/specs/2026-06-24-dakota-cluster-build-design.md`
- Modify: `AGENTS.md` (cluster topology table)

**Interfaces:**
- No code interfaces — documentation only

- [ ] **Step 1: Update design spec cluster topology**

In `docs/superpowers/specs/2026-06-24-dakota-cluster-build-design.md`, replace the Cluster Topology table with:

```markdown
## Cluster Topology

| Node     | CPUs | RAM    | BST eligible | Notes                                      |
|----------|------|--------|--------------|------------------------------------------  |
| ghost    | 32   | 62.5Gi | yes          | casd PVC lives here                        |
| bazzite  | 12   | 30.5Gi | yes          | overflow build pods                        |
| bluefin  | 16   | 31.2Gi | yes          | hamilton workstation (added June 2026)     |
| exo-1    | 22   | 15.1Gi | no           | excluded by 16Gi request (too little RAM)  |

exo-1 excluded automatically — `requests.memory=16Gi` exceeds exo-1's 15.1Gi allocatable.
```

Also update the resource management section — change `requests.memory: 12Gi` to `16Gi`:

```yaml
resources:
  requests:
    cpu: "4"
    memory: 16Gi   # 16Gi = minimum BST needs; also excludes exo-1 (15.1Gi) automatically
  limits:
    cpu: "8"
    memory: 28Gi
```

- [ ] **Step 2: Add bluefin/hamilton and exo-1 to AGENTS.md cluster topology table**

In `AGENTS.md`, find the Cluster Topology table and add the two new rows:

```markdown
| bluefin | k3s worker | 192.168.1.x | hamilton workstation — 16c/31Gi |
| exo-1   | k3s worker | 192.168.1.130 | 22c/15Gi — low RAM, skip BST builds |
```

- [ ] **Step 3: Commit**

```bash
cd /var/home/jorge/src/testing-lab
git add docs/superpowers/specs/2026-06-24-dakota-cluster-build-design.md AGENTS.md
git commit -m "docs: update node topology — add bluefin/hamilton, exo-1 ready"
git push
```
