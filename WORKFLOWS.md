# WorkflowTemplates — Agent Contract

This is the canonical contract for the reusable WorkflowTemplates that drive the
lab. Routine human entry points still come from the `Justfile` and the submit
workflows under `argo/`; this file documents the template layer they wrap. No
`kubectl apply`, no SSH.

Conventions:

- All templates live in `argo/workflow-templates/*.yaml` and are reconciled
  to namespace `argo` by the ArgoCD `testing-lab` Application.
- The usual operator entry points are `just run-tests`, `just run-tests-tag`,
  `just run-developer-tests`, `just run-software-tests`,
  `just run-tests-matrix`, `just run-titan-smoke`, `just run-titan-developer`,
  `just run-titan-software`, and `just run-flatcar-smoke`.
- Workflow-level parameters listed below are passed via `-p name=value`.
- Wall-clock targets are warm-cache numbers; cold-cache figures (BIB build
  on a missing golden disk) add ~5–10 min.
- The agent contract: prefer the **top-level** templates (`bluefin-qa-pipeline`,
  `bluefin-titan-smoke`). The supporting templates (provision, run, teardown)
  are called as `templateRef` and rarely submitted directly.

---

## Top-level entry points

### `bluefin-qa-pipeline`

Full pipeline: ensure golden disk → reflink + boot a fresh KubeVirt VM →
run test suites → teardown VM on exit.

| Parameter | Default | Notes |
|---|---|---|
| `image` | `ghcr.io/ublue-os/bluefin` | Source image. Tag is appended from `image-tag` for some callers; pass with tag if invoking directly. |
| `image-tag` | `latest` | `latest`, `lts`, etc. Also used as the golden-disk dir name. |
| `namespace` | `bluefin-test` | KubeVirt VM namespace. Use `bluefin-lts-test` for LTS. |
| `suites` | `smoke,developer` | Comma list; valid: `smoke`, `developer`, `software`. |
| `variant` | `bluefin` | Selects test fixtures (e.g. `dakota` for Ghostty). |
| `ssh-key-secret` | `bluefin-test-ssh-key` | Secret in `argo` ns with `id_ed25519`. |
| `branch` | `main` | Git ref cloned by the runner pod for test code and steps. |

Wall-clock: ~5 min (warm), ~10–14 min (cold BIB rebuild).

```
argo submit --from workflowtemplate/bluefin-qa-pipeline \
  -p image-tag=latest -p suites=smoke --wait
```

### `bluefin-titan-smoke`

Runs smoke tests against the **persistent** titan VMs (`titan-bluefin`,
`titan-lts`). Skips BIB and VM provisioning entirely. Use when iterating on
tests or when BIB is slow/broken.

Prerequisites: both titan VMs running. Fetch IPs:

```
kubectl get vmi titan-bluefin -n bluefin-test -o jsonpath='{.status.interfaces[0].ipAddress}'
kubectl get vmi titan-lts    -n bluefin-lts-test -o jsonpath='{.status.interfaces[0].ipAddress}'
```

| Parameter | Default | Notes |
|---|---|---|
| `vm-ip-latest` | *(required)* | titan-bluefin IP |
| `vm-ip-lts` | *(required)* | titan-lts IP |
| `suite` | `smoke` | Single suite name. |
| `ssh-key-secret` | `bluefin-test-ssh-key` | |
| `issue-title` | `titan smoke run` | Free-text label, appears in pod annotation. |
| `branch` | `main` | Git ref cloned by the runner pod. |

Wall-clock: ~5 min (preflight + test-only, no provisioning).

```
argo submit --from workflowtemplate/bluefin-titan-smoke \
  -p vm-ip-latest=10.42.x.y -p vm-ip-lts=10.42.x.z --wait
```

### `patch-golden-disk`

One-shot maintenance: re-runs the disk configuration step (SSH key,
selinux=0, sudoers) on an existing golden disk without rebuilding it.

| Parameter | Default | Notes |
|---|---|---|
| `image-tag` | `latest` | Disk dir under `/var/tmp/bluefin-golden/`. |

---

### `homelab-substrate`

Runs the first **in-cluster** homelab substrate lane. This is the k8s-first
counterpart to the VM-backed Bluefin workflows: it creates an ephemeral
namespace, deploys a simple workload on the cluster, runs plain pytest checks,
and tears the namespace down on exit.

| Parameter | Default | Notes |
|---|---|---|
| `lane` | `homelab-substrate` | Workflow label used for discovery/logging. |
| `branch` | `main` | Git ref cloned by the in-cluster pytest runner. |

Submit via:

```
argo submit argo/homelab-substrate.yaml -p branch=main --wait
```

Artifacts/evidence are emitted to pod stdout and `/tmp/results/` inside the
runner pod; the initial suite captures deployment status, service endpoints, pod
identity before/after restart, and rollout status.

### `bluefin-service-catalog-pipeline`

Runs the first **k8s-first** service-catalog lane. This creates an ephemeral
namespace, deploys a lane-specific workload with local-path-backed state, runs
plain pytest checks, and deletes the namespace on exit.

| Parameter | Default | Notes |
|---|---|---|
| `lane` | `media` | `media` or `nonmedia` |
| `branch` | `main` | Git ref cloned by the runner pod |

Submit via:

```
argo submit argo/bluefin-service-catalog-smoke.yaml -p lane=media --wait
argo submit argo/bluefin-service-catalog-smoke.yaml -p lane=nonmedia --wait
```

The initial implementation keeps both lanes inside Kubernetes and uses
local-path-backed PVC state. Hardware-heavy follow-ups remain separate issues.

### `homelab-access-probe`

Runs the first k8s-hosted HTTPS probe lane. It creates an ephemeral namespace,
deploys a TLS-enabled access fixture, runs hostname/routing/HTTPS checks, and
deletes the namespace on exit.

Submit via:

```
argo submit argo/homelab-access-probe.yaml --wait
argo submit argo/homelab-auth-probe.yaml --wait
```

### `homelab-restore-drill`

Runs the first local-path-backed restore drill. It creates an ephemeral
namespace, deploys a stateful workload, runs backup/restore verification, and
deletes the namespace on exit.

Submit via:

```
argo submit argo/homelab-restore-drill.yaml --wait
```

### `homelab-storage`

Runs the first in-cluster local-path storage lane. It creates an ephemeral
namespace, deploys a PVC-backed workload, validates persistence and
observability, and deletes the namespace on exit.

Submit via:

```
argo submit argo/homelab-storage.yaml --wait
```

---

## Supporting templates (called via `templateRef`)

These are exposed only because they are referenced by the entry points;
submit them directly only for diagnosis.

### `bib-build-and-push` (template: `ensure-disk`)

Builds the golden raw disk via `bootc-image-builder` if missing or stale.
Stale detection compares the upstream image digest (via skopeo) against the
`source-digest` marker written next to the disk on hostPath.

Outputs: no `outputs.parameters`; side effect is
`/var/tmp/bluefin-golden/<image-tag>/disk.raw` and `source-digest` on ghost.

### `provision-bluefin-vm` (template: `provision-vm`)

btrfs `cp --reflink=auto` from the golden disk, applies SVirt label, creates
a KubeVirt VM, waits for SSH/IP, emits `vm-ip` as an output parameter.

### `provision-flatcar-vm` (template: `provision-vm`)

Same shape for Flatcar — accepts an `ssh-pubkey` parameter directly instead
of relying on the bluefin-test secret for cloud-init injection.

### `run-gnome-tests` (template: `run-gnome-tests`)

`git-sync` initContainer clones testing-lab → main container SSHes to the VM
IP → installs deps (skipped if present) → runs qecore-headless + behave, plus
suite-local pytest files when `test_*.py` exists → captures the combined summary
to pod stdout and stores `results.json` / `pytest-results.xml` in `/tmp/results/`.

Resource limits and `hostNetwork: true` are set on the pod (KubeVirt
masquerade only routes from host netns).

### `run-flatcar-tests` (template: `run-flatcar-tests`)

Same shape for Flatcar; uses `core` as the SSH user and runs pytest+dogtail
fixtures from `tests/flatcar/`.

### `teardown-bluefin-vm` / `teardown-flatcar-vm`

Delete the VM, wait for the VMI object to drain, then `rm` the per-run
hostDisk clone. Invoked as `onExit` from the pipeline templates.

### `run-incluster-tests` (template: `run-pytest`)

Generic non-GNOME runner for k8s-first homelab lanes. It clones
`castrojo/testing-lab`, installs pytest + kubectl in the runner pod, executes a
repo test suite against in-cluster resources, and summarizes results to stdout.

### `run-service-tests` (template: `run-service-tests`)

Thin wrapper around `run-incluster-tests` for service-catalog lanes. It binds
the common service label and service name so media/non-media lanes can share one
runner surface.

### `homelab-substrate`

Creates an ephemeral namespace, deploys the first control-node workload
fixture, waits for rollout success, calls `run-incluster-tests`, then deletes
the namespace on exit.

### `bluefin-service-catalog-pipeline`

Creates an ephemeral namespace, deploys the selected service-catalog lane
fixture (`media` or `nonmedia`), calls `run-service-tests`, then deletes the
namespace on exit.

### `homelab-access-probe`

Creates an ephemeral namespace, generates a short-lived TLS secret, deploys the
access fixture, runs plain pytest checks against the HTTPS endpoint, then
deletes the namespace on exit.

### `homelab-restore-drill`

Creates an ephemeral namespace, deploys the restore fixture with a local-path
PVC, runs the backup/restore pytest suite, then deletes the namespace on exit.

### `homelab-storage`

Creates an ephemeral namespace, deploys the storage fixture with a local-path
PVC, runs the storage persistence/observability pytest suite, then deletes the
namespace on exit.

---

## CronWorkflows

Lives in `manifests/`, applied via the `testing-lab-infra` ArgoCD app:

| Schedule | Cron | Template called | Purpose |
|---|---|---|---|
| `nightly-smoke` | 02:00 UTC | `bluefin-qa-pipeline` (latest) | Catch upstream regressions |
| `nightly-smoke-lts` | 02:30 UTC | `bluefin-qa-pipeline` (lts) | Same for LTS; first fire builds the disk if it is absent |
| `orphan-vm-cleanup` | every 2h | inline | GC orphaned test VMs and their per-run hostDisk clones. **Titan-safe** (skips `app=titan-*`), live-workflow-safe, age ≥ 3h |
| `golden-disk-gc` | 04:00 UTC      | inline          | GC stale golden disks under `/var/tmp/bluefin-golden/`. Defaults to `DRY_RUN=true` |

Operational ops for CronWorkflows (suspend / resume / backfill) live in
[`docs/lab-operations.md`](docs/lab-operations.md) §8.6.

---

## Authoring a new lane

When you add a new lane (in-cluster homelab, service-catalog variant, or VM-backed
suite), follow this checklist so the lane stays consistent with existing ones and
agents can run it without reading the YAML:

1. **Tests:** add `tests/<lane>/test_*.py` (pytest) or `tests/<lane>/features/*.feature`
   (behave). Keep one assertion family per file.
2. **Fixture (if in-cluster):** `manifests/<lane>-fixture.yaml`. Server-side-apply friendly.
3. **Lane WorkflowTemplate:** `argo/workflow-templates/<lane>.yaml`. Required contract:
   - Creates an ephemeral namespace.
   - Applies the fixture (if any) and waits for rollout.
   - Calls `templateRef: run-incluster-tests` (in-cluster) or the smoke/provision/test/teardown
     sequence (VM-backed).
   - Deletes the ephemeral namespace in `onExit`.
4. **Submit wrapper:** `argo/<lane>.yaml` (top-level Workflow) that takes `branch`.
5. **Justfile recipe:** `just run-<lane>` that `argo submit`s the wrapper with `--watch`.
6. **Docs:** add a row to "Top-level entry points" above AND to
   [`docs/lab-operations.md`](docs/lab-operations.md) §3.1.
7. **ArgoCD:** nothing extra — both apps pick up new files in `argo/workflow-templates/`
   and `manifests/` on next sync.
8. **Validate:** push → wait for sync → `just run-<lane>` → check Loki for results.
9. **Schedule (optional):** only add a CronWorkflow once the lane has run green at least
   once on demand.

---

## Editing this contract

When you add or rename a template, update this file in the same PR. Drift
between templates and this doc is what breaks autonomous agents.
