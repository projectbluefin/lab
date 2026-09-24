---
name: argo-patterns
description: >
  Recurring Argo Workflows patterns for QA, publishing, and scheduling.
metadata:
  context7-sources:
    - /argoproj/argo-workflows
    - /oras-project/oras
    - /websites/github_en_rest
    - /kubevirt/user-guide
---

# Argo Workflows Common Patterns

Topic-grouped reference of hard-won lab patterns. Load after
[SKILL.md](SKILL.md) when authoring or debugging workflows;
[authoring.md](authoring.md) covers template structure rules. Registry and
base-image rules (including the ban on installing packages at container
runtime) live in
[`gitops-argocd/image-policy.md`](../gitops-argocd/image-policy.md).

Groups: image sync and pollers · concurrency and scheduling · publishing
results back to GitHub · conditionals and DAG logic · CronWorkflows · pods,
storage, and operations · BuildStream pipelines.


## Image sync and pollers

### Authenticated digest-preserving registry publication

For a scheduled Zot-to-GHCR publisher, keep the reusable logic in a
WorkflowTemplate and point the CronWorkflow at it. Mount an operator-managed
`kubernetes.io/dockerconfigjson` Secret as an auth file; do not expose registry
credentials through parameters, command arguments, stdout, or shell tracing.

Resolve the source tag to a digest, copy `source@digest`, and inspect the
destination tag with the same auth file. A lane succeeds only when the
destination digest exactly matches the source digest. Put a semaphore on the
entrypoint DAG so separate publication workflows serialize while independent
image lanes within one workflow can still run in parallel.

Use terminal-status dependencies for the result task:

```yaml
depends: >-
  (lane-a.Succeeded || lane-a.Failed || lane-a.Errored) &&
  (lane-b.Succeeded || lane-b.Failed || lane-b.Errored)
```

Pass each `{{tasks.<lane>.status}}` to the result task and fail it unless every
lane succeeded. Bound each lane with `retryStrategy.limit` and backoff.

After the image copy, use `oras discover` against the source digest. If
referrers exist, recursively copy them with the destination registry auth file.
Log explicit `none present` or `discovery unavailable` states as non-fatal;
failure to copy referrers that were discovered is a lane failure. Verify the
destination digest after this optional evidence step.

Do not rely on `lab-runner:latest` for registry clients — verified (2026-08) to
contain bash, curl, git, jq, python3, and kubectl, but **not** skopeo, oras, or
tar. Use images that already carry the client at a pinned version:
digest-pinned `quay.io/skopeo/stable` for shell+skopeo steps (or the distroless
org `ghcr.io/projectbluefin/skopeo` for shell-free `container:` steps) and
digest-pinned `ghcr.io/oras-project/oras` for referrer work, as the live
`zot-candidate-lifecycle` template does. Never bootstrap a tool by downloading
it at pod start — a runtime download is an ungoverned, offline-fragile
dependency (see [`gitops-argocd/image-policy.md`](../gitops-argocd/image-policy.md)).
If a referrer client is genuinely unavailable to a lane, log the missing
capability explicitly and continue with digest verification.

```bash
# Source: Context7 /oras-project/oras
oras discover --plain-http --format json "${SOURCE}@${DIGEST}"
oras cp --recursive --from-plain-http \
  --to-registry-config /auth/config.json --no-tty \
  "${SOURCE}@${DIGEST}" "${DESTINATION}:testing"
```

For durable run history, attach a root `onExit` template and pass
`{{workflow.status}}`, `{{workflow.name}}`, and
`{{workflow.creationTimestamp.RFC3339}}` through environment variables. Build a
compact record with `jq`, then invoke the validated publisher CLI. Do not append
`|| true`: persistence is part of the workflow contract, so an exit-handler
failure must remain visible and make an otherwise successful run fail. Never
put the GitHub token in a clone URL or command argument; expose it only as
`GITHUB_TOKEN` from a Secret and let the CLI's `GIT_ASKPASS` path consume it.

> Source: Context7 `/argoproj/argo-workflows` exit-handler and global-variable
> documentation. Exit handlers always run and receive the terminal workflow
> status; `workflow.creationTimestamp.RFC3339` is available in the exit handler.

### Dakota BuildStream publish lane output tags

`dakota-build-pipeline` exports the built `oci/bluefin.bst` artifact to the local Zot
registry. NVIDIA variants are disabled in the distributed clean-build workflow. The published
tag must match the projectbluefin/dakota image contract:

- Base variant → `<lab-ip>:30500/dakota:testing`

Do not publish these as `:latest` from the cluster lane; `:testing` is the testing-branch
stream and `:stable` is promoted separately from `main`. Keeping the cluster lane on `:testing`
prevents accidental overwrites of the stable/production stream and makes the artifact identity
obvious to downstream lab jobs.

### Dakota verification: containerized QA when the VM path is blocked

Dakota images are built from a composefs-oci backend that declares `bootloader = "systemd"` but
does not ship a UKI. The lab's standard VM QA path (containerDisk → `bootc install to-disk`
→ KubeVirt VM) therefore fails with `bootupd is required for ostree-based installs` because bootc
1.16.2 bails for systemd-boot ostree installs when no UKI is present. Until Dakota ships a UKI
(or bootc gains a composefs-oci install path), VM-boot verification is blocked.

WorkflowTemplate: `dakota-container-qa-pipeline`

- Runs image-level smoke checks directly inside a pod built from the target OCI image.
- Verifies Dakota identity (`/etc/os-release`), presence of key binaries (`podman`, `flatpak`,
  `gnome-shell`, `bootc`), bootc install config, and valid `bootc status` JSON.
- Requires no `bootc install` and no containerDisk.
- GUI behave suites (`smoke`/`developer` via `qecore-headless`) cannot run inside a pod because
  `qecore-headless` requires a full systemd/GDM session.

Default invocation for a fresh `dakota:testing` build:

```bash
argo submit --from workflowtemplate/dakota-container-qa-pipeline \
  -p image=<lab-ip>:30500/dakota \
  -p image-tag=testing \
  -p variant=dakota \
  -n argo --watch
```

NVIDIA image builds are disabled for this clean distributed-build path.

`dakota-qa-pipeline` has since migrated to the container-only path — it invokes
`run-container-tests` directly and takes no KubeVirt or containerDisk inputs,
and `image-poll-dakota` (not suspended) drives it on every new `dakota:testing`
digest.

## Concurrency and scheduling

### VM concurrency: k8s native scheduling, no semaphores

VM concurrency is managed by the **k8s scheduler via virt-launcher pod memory requests**, not Argo semaphores. When a node has insufficient RAM, the virt-launcher pod stays Pending. When a VM finishes, resources free up and the scheduler picks the next Pending pod. FIFO ordering follows workflow creation timestamp.

**Do not add `spec.synchronization.semaphores` to VM pipeline specs.** The semaphore approach was removed because:
- Slots were held at workflow scope, not VM-live scope — a 3h pipeline held a slot during build/assert/teardown, not just while the VM was running
- Build workflows (no VM) held VM slots, starving actual test workflows
- The slot count was a manually-maintained number that drifted from actual hardware

**What to do instead:** ensure VM specs have explicit memory requests so the scheduler has accurate data:
```yaml
domain:
  memory:
    guest: "{{inputs.parameters.vm-memory}}"   # KubeVirt sets virt-launcher request from this
```

**All pipelines still need `activeDeadlineSeconds`** so stuck VMs self-evict:
```yaml
activeDeadlineSeconds: 3600   # 1h
```

**VMs float to any KubeVirt-capable node** — no `nodeSelector: kubernetes.io/hostname: ghost` in VM specs. The registry-mirror-config DaemonSet writes the Zot HTTP registry config to all nodes.

### Semaphore topology: hold the key at one level, cap every fan-out

Two rules govern every ConfigMap-backed semaphore in `manifests/workflow-semaphores.yaml`.
`tests/unit/test_semaphore_topology.py` enforces them in `just lint`.

**Rule 1 — declare the key on the leaf that consumes the resource, never on
`spec.synchronization`.** A workflow-level semaphore is held for the *entire*
run, including the orchestration time before and after the real work. If a
parent holds `ghost-container-qa` while its `test-lane` children also need it,
the parent starves its own children and nothing can ever make progress:
the holder is waiting on the resource it is holding. `ghost-container-qa` is
declared exactly once, on `run-container-tests/run-container-tests`.

**Rule 2 — `spec.parallelism` does NOT travel through `templateRef`.** This is
the trap. Only a *spec-level* `workflowTemplateRef` inherits workflow-level
fields (`parallelism`, `activeDeadlineSeconds`, `workflowMetadata`). A
`templateRef` inside a `dag.tasks[]`/`steps[]` entry imports the single named
template and nothing else.

```yaml
# a poller DAG task like this:
- name: qa-dakota
  templateRef:
    name: dakota-qa-pipeline    # spec.parallelism: 2 is SILENTLY DROPPED
    template: pipeline
```

So a spec-level `parallelism: 2` protected only direct submissions.
Poller-dispatched runs fanned out all five `withItems` lanes at once and a
single workflow held 5 of the 6 `ghost-container-qa` slots. Fix: put
`parallelism` on the **template** that fans out, where it survives `templateRef`:

```yaml
templates:
  - name: pipeline
    parallelism: 2          # survives templateRef; keep it < the semaphore limit
    dag:
      tasks:
        - name: test-lane
          withItems: [smoke, common, developer, software, system]
          templateRef: { name: run-container-tests, template: run-container-tests }
```

Invariant to preserve: `per-template parallelism < semaphore limit`, so at
least two distinct workflows can always progress and no single pipeline can
monopolise the runner. Raising the semaphore limit is never a fix on its own —
it only moves the threshold.

### Locking one DAG build task

`synchronization` is a template-level field; it is not valid on an individual
`dag.tasks[]` entry. When one task must share a cross-workflow semaphore with
another builder, invoke a local wrapper template from the DAG and put the
semaphore on that wrapper:

```yaml
tasks:
  - name: build
    template: serialized-build
templates:
  - name: serialized-build
    synchronization:
      semaphores:
        - configMapKeyRef:
            name: workflow-semaphores
            key: bst-build
    steps:
      - - name: invoke-builder
          templateRef:
            name: shared-builder
            template: build
          arguments:
            parameters:
              - name: image
                value: "{{workflow.parameters.image}}"
```

This keeps the lock around only the build task while preserving the
cross-WorkflowTemplate `templateRef`. For diagnostic collection, include all
terminal upstream states when appropriate:
`(tests.Succeeded || tests.Failed || tests.Errored)`.

### Bound BuildStream admission before the semaphore queue

The `bst-build` semaphore limits execution to one pipeline, but a semaphore by
itself permits an unbounded list of waiting workflows. Automated callers must
also count active workflows labeled `bluefin.io/bst-workload=true` and defer
when two are already admitted: one may execute while one waits.

Source pollers must persist a new commit
SHA only after the referenced build succeeds, so deferred or failed work is
retried.

## Publishing results back to GitHub

### Contents API vs standalone git push-back

When a workflow pod needs to push a simple file to a GitHub repo, use `curl` + `jq` inside the bash script (Contents API).

However, for complex updates (such as parsing BDD/behave test results, merging with historical runs, and capping the history), **never use inline python or complex inline bash blocks**. Instead, extract the logic into a **standalone Python script** inside the repository, clone the repository dynamically within the container using `GITHUB_TOKEN`, and run the script locally to perform a standard git transaction (`git clone` → update → `git commit` → `git push`).

**Contents API Pattern (for simple single-file writes, verified against Context7 `/websites/github_en_rest`):**
```bash
# GET current file sha (required for updates)
CURRENT=$(curl -sf \
  -H "Authorization: token ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/OWNER/REPO/contents/PATH/file.json" || echo "{}")
FILE_SHA=$(echo "$CURRENT" | jq -r '.sha // empty')

# Build payload with jq — no Python, no heredocs
CONTENT=$(echo "$PAYLOAD_OBJ" | base64 -w0)
BODY=$(jq -nc \
  --arg msg "commit message" \
  --arg content "$CONTENT" \
  --arg sha "$FILE_SHA" \
  'if $sha != "" then {message:$msg,content:$content,sha:$sha} else {message:$msg,content:$content} end')

# PUT — sha required for updates, omit for new files
HTTP_CODE=$(curl -sf -w "%{http_code}" -o /tmp/response.json \
  -X PUT \
  -H "Authorization: token ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -H "Content-Type: application/json" \
  -d "$BODY" "https://api.github.com/repos/OWNER/REPO/contents/PATH/file.json")
```

Key rules:
- `sha` field required when updating an existing file; omit for new files (404 on GET = new file)
- `content` must be base64 encoded; use `base64 -w0` (no line wraps)
- `X-GitHub-Api-Version: 2022-11-28` header required by current GitHub API
- Retain output through Argo logs/artifacts or a workflow PVC — never a
  root-backed hostPath
- Concurrent pipeline exits conflict on SHA → last writer wins; 409 = silent skip. Acceptable for metrics files.

#### Container-only QA runner

When starting the nested target with Podman, pass `/sbin/init` explicitly after the
image reference. Some bootc OCI images have an empty image `Cmd`; relying on
`--systemd=always` alone then makes crun fail with `cannot find `` in $PATH` before
systemd starts.

**Never install tooling at container runtime** (`dnf install`, `apt-get install`, `pip install`,
`curl | sh`) — that is a banned antipattern, per
[`gitops-argocd/image-policy.md`](../gitops-argocd/image-policy.md). When a step needs a tool
its image does not carry, switch the step to an org-published image that already has it:

- `ghcr.io/projectbluefin/lab-runner:latest` — shell-enabled CI utility steps: bash, curl,
  git, jq, python3, kubectl. Verified 2026-08 by running the image: it does **not** contain
  skopeo, oras, or tar.
- `ghcr.io/projectbluefin/skopeo:latest` — distroless skopeo (1.23.0 at `/usr/bin/skopeo`,
  no shell): invoke with explicit `command:`/`args:` on a `container` template, or keep using
  the digest-pinned `quay.io/skopeo/stable` when the step needs a shell next to skopeo.

**Why no inline Python or heredocs (root cause):** YAML `source: |` literal blocks use indentation to determine block extent. Any line at column 0 (including unindented `python3 -c "...\nimport json\n..."` continuation lines, or heredoc bodies like `<<'EOF'\nimport json\n`) terminates the block — YAML treats those lines as new top-level keys. The `yaml: could not find expected ':'` error is the symptom. Fix: use `jq` one-liners, keep everything on the same indented line, or `--rawfile` to read from a pre-staged file.

## Conditionals and DAG logic

### The when/Skipped output trap: never reference a Skipped task's outputs

**Verified against Context7 `/argoproj/argo-workflows` enhanced-depends-logic docs:**
> "If a downstream task references outputs from a task that was Skipped or Omitted,
> those references will resolve to empty strings."

So `'{{tasks.check.outputs.result}}' != 'exists'` becomes `'' != 'exists'` = `true` when
the upstream is Skipped. In theory the build should run. In practice (Argo v4.0.5), we
observed the downstream task never being scheduled at all — the controller logs show
`"was unable to obtain the node"` for it and the workflow stalls with only 2 nodes.

The safe, version-independent fix: **never put a `when` guard on the task that owns the
gate output. Move the bypass inside the script.**

**Example of the fragile pattern:**
```yaml
- name: check
  when: "'{{inputs.parameters.force}}' != 'true'"   # Skipped when force=true
  template: check

- name: build
  depends: "(check.Succeeded || check.Skipped)"
  when: "'{{tasks.check.outputs.result}}' != 'exists'"  # resolves to '' when Skipped → unpredictable
  template: build
```

**The robust fix:**
```yaml
- name: check           # always runs — no 'when' on this task
  template: check       # script handles force=true internally

- name: build
  depends: "check.Succeeded"
  when: "'{{tasks.check.outputs.result}}' != 'exists'"   # ✅ always defined
  template: build
```

Inside the `check` script:
```bash
if [[ "{{inputs.parameters.force}}" == "true" ]]; then
  echo "missing"   # short-circuit — always rebuild
  exit 0
fi
# … real existence check …
```

**Rule:** If a task has a `when` guard AND downstream tasks reference its outputs,
remove the `when` guard and move the bypass into the script body.

**Symptoms of this bug:**
- Workflow shows phase `Running` but only 1–2 nodes (the DAG + the Skipped task)
- No `install-to-disk` or equivalent node ever created
- Controller logs show `"was unable to obtain the node"` for the downstream task (normal reconciliation noise)
- `force=true` workflows submitted after a digest change never actually build

### when values with hyphens must be quoted or avoided

Argo's `when` expression parser (expr-lang based) treats an unquoted hyphenated
string as a subtraction expression. A condition like:

```yaml
when: "{{inputs.parameters.mode}} == cache-only"
```

expands to `cache-only == cache-only`, which fails with:

```
Value 'cache' cannot be used with the modifier '-', it is not a number
```

**Fixes:**

1. Quote the literal: `when: "{{inputs.parameters.mode}} == 'cache-only'"` works
   for values that expr-lang can parse as a single quoted string.
2. Safer: avoid hyphens in the enumerated value entirely. Use `local` instead of
   `cache-only` and quote the comparison: `when: "{{inputs.parameters.mode}} == 'local'"`.

Always lint after changing `when` expressions, then submit a test workflow to
verify the DAG branches are scheduled as expected before relying on the path in
production.

## CronWorkflows

### CronWorkflow uses schedules, not schedule

CronWorkflow uses `schedules` (plural array), not `schedule` (singular string). The singular field does not exist in the CRD schema — ArgoCD's ServerSideApply validation will reject it.

```yaml
# ✗ WRONG — rejected by ArgoCD schema validation
spec:
  schedule: "0 * * * *"

# ✅ CORRECT
spec:
  schedules:
    - "0 * * * *"
```

Verified against Context7 `/argoproj/argo-workflows` CronWorkflow spec docs.

CronWorkflows also cannot be invoked via `workflowTemplateRef` — if you need a CronWorkflow to be submittable manually, extract its logic into a WorkflowTemplate and have the CronWorkflow reference it with `workflowTemplateRef`.

### CronWorkflow suspend can survive a git removal: verify live state

Removing `spec.suspend: true` from a CronWorkflow's git manifest and syncing does **not**
reliably clear the live field, even when ArgoCD reports the resource `Synced` and the sync
`operationState` says `Succeeded`. This was observed directly: after removing `suspend: true`
from 10 CronWorkflow manifests, committing, pushing, and force-syncing (`annotate
argocd.argoproj.io/refresh=hard`), ArgoCD reported all 10 as `Synced` — but `kubectl get
cronworkflow <name> -o jsonpath='{.spec.suspend}'` still returned `true` on every one of them.

**Always verify the live field directly after removing it from git — never trust the
ArgoCD sync/resource status alone for boolean fields that may have been set by a prior
apply.** If live state doesn't match git after a confirmed sync, patch it directly:

```bash
kubectl patch cronworkflow -n argo <name> --type=merge -p '{"spec":{"suspend":false}}'
```

Root cause not conclusively identified (suspected Server-Side Apply field-ownership —
a boolean field set by an earlier field manager isn't cleared just because a later
manifest omits it). Treat any boolean/scalar field removal from a CronWorkflow the same
way: confirm live state with `kubectl get -o jsonpath`, don't stop at "ArgoCD says Synced".

## Pods, storage, and operations

### Per-workflow ephemeral storage: volumeClaimTemplates

For pipelines that need shared scratch space across steps (e.g. installer binaries, target disks),
use Argo's `volumeClaimTemplates` at the workflow spec level. Argo auto-creates the PVC at workflow
start and auto-deletes it on completion — no manual cleanup step needed.

```yaml
spec:
  volumeClaimTemplates:
    - metadata:
        name: workspace
      spec:
        accessModes: ["ReadWriteOnce"]
        storageClassName: local-path   # explicit non-root node mapping in GitOps
        resources:
          requests:
            storage: 30Gi

  templates:
    - name: my-step
      script:
        volumeMounts:
          - name: workspace
            mountPath: /mnt/workspace
```

**RWO PVC + KubeVirt VM co-location:** When a VM uses a `persistentVolumeClaim` volume backed
by a RWO PVC, KubeVirt automatically schedules the VM on the same node as the PVC — no explicit
`nodeSelector` needed. Source: /kubevirt/user-guide — "When using local devices or ReadWriteOnce
(RWO) PVCs, affinity rules on VMs sharing storage ensure they are scheduled on the same node."

The local-path provisioner configuration must contain an explicit non-root data
mount for every eligible node. It has no default path: a PVC on an unconfigured
node must fail provisioning rather than write to the root filesystem.

**Namespace constraint:** `volumeClaimTemplates` creates the PVC in the workflow's own namespace
(`argo`). If a VM in a different namespace (e.g. a per-run VM namespace) needs a disk, create a dedicated PVC
in that namespace via a `resource:` step, and delete it in `onExit`.

```yaml
# registry-lint-ignore not needed — no image ref
- name: create-rootdisk-pvc
  resource:
    action: apply
    manifest: |
      apiVersion: v1
      kind: PersistentVolumeClaim
      metadata:
        name: "{{workflow.name}}-rootdisk"
        namespace: <vm-namespace>
      spec:
        accessModes: [ReadWriteOnce]
        storageClassName: local-path
        resources:
          requests:
            storage: 30Gi
```

**containerDisk OCI format** (source: /kubevirt/user-guide):
```dockerfile
FROM scratch
ADD --chown=107:107 disk.raw /disk/
```
UID 107 = qemu. Required — omitting `--chown` causes VM boot failure (permission denied on disk).

### Configure registry mirrors and signature policy before container builds

When running `podman build`, `bootc install`, or other image/pull operations inside a privileged Argo workflow container, you must configure any custom registries mirror files (such as `/etc/containers/registries.conf.d/bluefin-local-zot.conf` to hook up the local Zot pull-through cache) and security policy files (such as `/etc/containers/policy.json`) BEFORE executing those container operations. 

In particular, if the base image being pulled or built has a strict production signature policy built into its `/etc/containers/policy.json` (as is the case with Bluefin production images), `bootc install` and other podman/skopeo pull tasks will reject pulling unsigned images from local registries or GHCR with exit code 125 ("Source image rejected: A signature was required, but no signature exists"). Overwriting the pod container's local `/etc/containers/policy.json` with an insecure policy (e.g. `"type": "insecureAcceptAnything"`) prevents this exit-125 failure.

This is extremely critical to understand if a workflow ever uses `hostPID: true`. If a pod using `hostPID: true` exits with failure (or is terminated/timed out), the `argoexec` process teardown signals all processes in its view — which in a host PID namespace means **every host process**, killing host daemons like `k3s`, `sshd`, and `systemd-journald` and crashing the node. Therefore, `hostPID: true` and `hostIPC: true` must NOT be used in build containers. Bypassing signature checks using `policy.json` prevents exit-125 crashes, but removing `hostPID` entirely is the primary safety guarantee.

**Correct order of execution:**
1. Configure containers-storage graphroot.
2. Write registry mirror configuration files under `/etc/containers/registries.conf.d/`.
3. Overwrite `/etc/containers/policy.json` with `insecureAcceptAnything` to bypass signature checks.
4. Run container build or install operations (e.g., `podman build --tls-verify=false -t ...` or `bootc install to-disk ...`).
5. Run container push operations (e.g., `podman push ...`).

### /tmp permission denied for non-root containers

If an Argo workflow container template is configured to run as a non-root user (such as `runAsUser: 1000` in `run-container-tests.yaml`), and needs to write results, temporary configurations, or scripts under `/tmp`, it can easily fail with `Permission denied` (exit code 1). This happens because `/tmp` inside the bootc rootfs image is typically owned by root with restricted permissions.

The clean, standard Kubernetes/Argo solution is to mount an `emptyDir: {}` volume on `/tmp` inside the pod container. This provides a fresh, fully-writable `/tmp` filesystem that is owned by the executing non-root user (1000) and completely isolates test execution from any image-baked `/tmp` permission constraints.

**Implementation pattern:**
```yaml
    container:
      image: "{{inputs.parameters.image}}:{{inputs.parameters.image-tag}}"
      securityContext:
        runAsUser: 1000
        runAsGroup: 1000
      volumeMounts:
        - mountPath: /tmp
          name: tmp
    volumes:
      - name: tmp
        emptyDir: {}
```

### Log access: Argo is sufficient, no separate stack needed

Argo Server retains all workflow pod logs for the workflow TTL period (7 days success,
30 days failure via `workflow-controller-configmap`). No separate log aggregation stack
(Loki, Promtail, etc.) is needed for a homelab CI cluster.

**Retrieve logs:**
```bash
# most recent workflow
just logs                              # alias: argo logs -n argo @latest

# specific workflow
argo logs -n argo <workflow-name>

# specific pod/container
kubectl logs -n argo <pod> -c main

# via MCP
argo-mcp-logs_workflow <workflow-name>
```

**Why a separate log stack is redundant:**
- Pod logs are already captured and served by the Argo Server
- Artifacts (`results.json`, `atspi_tree.txt`) echo to stderr — accessible via `argo logs`
- Cross-workflow queries → `argo list -n argo` then `argo logs` per workflow
- Adding Loki + Promtail duplicates storage, adds 2–3 pods, and a 10Gi PVC for no
  additional capability that `argo logs` doesn't already provide

## BuildStream pipelines

### BuildStream resource right-sizing and scheduler-driven affinities

When designing or updating BuildStream compilation pipelines (e.g. `dakota-build-pipeline` and `bluefin-server-build-pipeline`), right-size all step-level resource requests and limits to maximize cluster capacity and prevent scheduling bottlenecks:

- **RE Coordinator/Driver Pods**: The remote execution build driver (e.g., `bst-build-re`) only orchestrates execution, downloads metadata, and transfers sparse artifact layers; its native CPU/memory usage is minimal (~47m CPU, ~926Mi memory). Keep its resource requests right-sized at `2 CPU` and `4Gi` memory (with limits at `4 CPU` and `8Gi` memory) to prevent massive node capacity stranding.
- **Local/Serial Builder Pods**: Local compile templates (e.g. `bst-build-local`) can spike up to 15.9 CPU cores but rarely exceed ~9.6GiB of memory and ~0.36GiB of container-overlay filesystem storage (since the BuildStream artifact cache is mapped directly to a hostPath or PVC). Right-size requests to `16 CPU` and `16Gi` memory with a `10Gi` ephemeral storage request (limits: `32 CPU`, `32Gi` memory, `50Gi` ephemeral storage) to avoid stranding resources while leaving ample compiling headroom.
- **Preferred Node Affinities**: Avoid hard node pinnings (like `nodeSelector: kubernetes.io/hostname: exo-0`) on build templates. Instead, utilize a `preferredDuringSchedulingIgnoredDuringExecution` preferred node affinity targeting the primary build node (e.g., `exo-0` with weight 100) to keep cache locality warm under normal conditions, while enabling the Kubernetes scheduler to gracefully schedule build pods onto other available nodes (such as `exo-1`) when the primary is overloaded or undergoing maintenance. This fully aligns with scheduler-driven placement policies.
