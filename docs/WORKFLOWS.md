# Workflow Reference

## Table of Contents
- [Pipelines](#pipelines)
  - [bluefin-qa-pipeline](#bluefin-qa-pipeline)
  - [dakota-qa-pipeline](#dakota-qa-pipeline)
  - [dakota-build-pipeline](#dakota-build-pipeline)
  - [cosmic-build-pipeline](#cosmic-build-pipeline)
  - [bluefin-server-build-pipeline](#bluefin-server-build-pipeline)
  - [bst-qa-pipeline](#bst-qa-pipeline)
  - [knuckle-qa-pipeline](#knuckle-qa-pipeline)
- [Supporting Templates](#supporting-templates)
- [Distributed Build/RE Grid](#distributed-buildre-grid)
- [Cache Warming (Pollers)](#cache-warming-pollers)
- [Nightly Schedule](#nightly-schedule)
- [Priority Classes](#priority-classes)
- [Resource Profiles](#resource-profiles)

## Pipelines

### bluefin-qa-pipeline
- **Purpose:** Test an already-built Bluefin/Bluefin-LTS containerDisk. Does **not**
  build anything itself — fails fast if the containerDisk is missing from Zot.
- **Parameters:** `image`, `image-tag`, `namespace`, `suites`, `variant`,
  `containerdisk-tag`, `ssh-key-secret`, `branch`, `pr-number`, `sha`, `repo`, `vm-memory`.
- **DAG:** `assert-cd` (skopeo inspect against Zot) → parallel `test-lane` items
  (`smoke`, `common`, `developer`, `software`, `system`, filtered by `suites`) →
  `onExit: cleanup-and-report` (deletes any orphaned lane VMs, posts a GitHub commit
  status on PR runs).
- **Each lane:** `provision-bluefin-vm` (containerDisk VM boot) → `run-gnome-tests`
  (SSH in, run `behave`) → `teardown-bluefin-vm`.
- **Who builds the containerDisk:** the `digest-watch` CronWorkflow (see
  [Cache Warming](#cache-warming-pollers)) — completely decoupled from this pipeline.
- **Just recipe:** `run-tests`, `run-tests-tag`, `run-tests-matrix`.

### dakota-qa-pipeline
- **Purpose:** Validate an existing Dakota containerDisk (same assert-cd/fail-fast
  model as bluefin-qa-pipeline).
- **Parameters:** `image`, `image-tag`, `containerdisk-tag`, `namespace`, `suites`,
  `variant`, `ssh-key-secret`, `branch`, `vm-memory`.
- **DAG:** `assert-cd` → parallel `test-lane` items (`smoke`, `developer`, `system`);
  `onExit: cleanup-orphan-vms`.
- **Just recipe:** `run-dakota-qa`.

### dakota-build-pipeline
- **Purpose:** The actual BuildStream compile step for Dakota — builds
  `oci/bluefin.bst` and `oci/bluefin-nvidia.bst` in parallel and pushes both to
  the local Zot registry. This is what populates/warms the Dakota build cache.
- **Distribution:** No `nodeSelector` pin — the k8s scheduler naturally spreads
  the two variant builds across ghost and exo-0 in parallel (confirmed live:
  one variant per node, both `Running` simultaneously).
- **Cache:** Uses `bst-artifact-server` (bazel-remote, gRPC `:9092`, `argo` namespace)
  as the BuildStream artifact cache — **not** Buildbarn, and **not**
  `buildbox-casd` (that was an unused, disconnected CAS daemon, deleted).
  Project-defined cache remotes are overridden so Dakota stays local-only in
  cluster (`override-project-caches: true`, artifact server pinned to
  `bst-artifact-server`, `source-caches.servers: []`).
  Requires privileged FUSE/mount-namespace access for real bootc OCI builds, so it
  cannot run through Buildbarn's chroot-only remote-execution workers.
- **Priority:** `priorityClassName: bst-build` — preemptable by `lab-test-vm`
  pods on resource contention.
- **Who triggers it automatically:** `dakota-commit-poller` (see
  [Cache Warming](#cache-warming-pollers)).

### cosmic-build-pipeline
- **Purpose:** BuildStream compile pipeline for COSMIC variants
  (`oci/cosmic/image.bst`, `oci/cosmic-nvidia/image.bst`) and push to local Zot.
- **Safety guards (aligned with dakota):**
  `activeDeadlineSeconds: 14400` (workflow), `activeDeadlineSeconds: 5400` (step),
  `retryStrategy: limit=2, retryPolicy=Always`, `GRPC_POLL_STRATEGY=poll`,
  `GRPC_ENABLE_FORK_SUPPORT=1`, `request-timeout: 900`,
  `scheduler.network-retries: 4`, `scheduler.fetchers: 1`.
- **Cache policy:** local-only cluster CAS (`bst-artifact-server`), with
  `override-project-caches: true` and empty source-caches.

### bluefin-server-build-pipeline
- **Purpose:** BuildStream compile pipeline for Bluefin Server elements
  (`oci/bluefin-server-ddi.bst`, `oci/bluefin-server-installer.bst`) and push to local Zot.
- **Safety guards (aligned with dakota/cosmic):**
  `activeDeadlineSeconds: 14400` (workflow), `activeDeadlineSeconds: 5400` (step),
  `retryStrategy: limit=2, retryPolicy=Always`, `GRPC_POLL_STRATEGY=poll`,
  `GRPC_ENABLE_FORK_SUPPORT=1`, `request-timeout: 900`,
  `scheduler.network-retries: 4`, `scheduler.fetchers: 1`.
- **Cache policy:** generated config now starts with `cache.storage-service` to
  `bst-artifact-server:9092` plus connection-config retries/timeouts, then enforces
  local-only artifact/source cache overrides for project and junction remotes.

### bst-qa-pipeline
- **Purpose:** Smoke-tests the Buildbarn distributed remote-execution grid itself
  by running a trivial BuildStream element through it.
- **Cache + RE wiring:** artifact cache via `bst-artifact-server:9092` (gRPC);
  project cache remotes overridden to local-only (`override-project-caches: true`,
  empty source-caches);
  remote-execution via the in-cluster Buildbarn frontend
  (`frontend.buildbarn.svc.cluster.local:8980`), distributed across ghost+exo-0
  workers. See [Distributed Build/RE Grid](#distributed-buildre-grid).
- **Known limitation:** the current test element (`hello.bst`, an `import` kind)
  proves config wiring (BuildStream connects to the frontend with no errors) but
  never actually dispatches an action through the scheduler to a worker — verified
  by checking the CAS blocks file (zero bytes written). A real build-dispatch
  test element would be needed to prove end-to-end RE execution conclusively.

### knuckle-qa-pipeline
- **Purpose:** Build the Knuckle installer ISO/binary, provision a blank VM, run
  the headless installer in-cluster, boot the installed system, rediscover SSH
  reachability, and run smoke tests.
- **Parameters:** `branch`, `namespace`, `suite`, `ssh-key-secret`, `tests-branch`.
- **DAG:** `clone-source` → `build-installer` → `provision-target-vm` →
  `boot-installer` → `wait-install-complete` → `transition-to-installed` →
  `discover-installed-ip` → `run-smoke-tests`; `onExit: teardown`
  (`teardown-vm` → `cleanup-installer-artifacts`).
- **Disk:** PVC-backed (`local-path`), not hostDisk/hostPath — KubeVirt
  co-schedules the VM automatically on the PVC's node, no explicit
  `nodeSelector` needed.
- **Just recipe:** None currently; submit the WorkflowTemplate directly or use
  the nightly CronWorkflow.

## Supporting Templates

| Template | Role |
| --- | --- |
| `build-containerdisk` | Shared containerDisk builder used by `digest-watch`/`image-poller`. Flow: `check` (Zot existence) → `install-to-disk` → `convert-and-push`. Builds a KubeVirt containerDisk from a bootc image and pushes it to the local registry. |
| `provision-bluefin-vm` | Shared Bluefin/Dakota VM bring-up directly from a containerDisk (no reflink, no golden disk, no hostDisk). `create-vm` defines a 4 vCPU KubeVirt VM (`vm-memory` param, default 8Gi), `wait-for-vm-ready` starts `sshd.socket` via QEMU guest agent and returns the pod IP once SSH is reachable. `priorityClassName: lab-test-vm`. |
| `provision-flatcar-vm` / `provision-gnomeos-vm` | Same containerDisk-VM pattern as above for their respective variants. Both set `priorityClassName: lab-test-vm`. |
| `teardown-bluefin-vm` / `teardown-flatcar-vm` / `teardown-gnomeos-vm` | Deletes the KubeVirt VM (and hostDisk/PVC where applicable). |
| `run-gnome-tests` | Shared test runner. Clones `projectbluefin/testsuite` (the single source of truth), waits for SSH, installs test dependencies, copies `tests/<suite>`, and runs `behave`. GUI suites (smoke) run via `qecore-headless` inside the VM. The `common` suite runs from the runner container with `VM_IP`/`SSH_KEY` exported so its SSH steps reach the VM directly — do NOT run common via qecore-headless. The `system` suite runs inside the VM without a display. |
| `run-incluster-tests` | Shared in-cluster pytest runner. Git-syncs `lab`, runs a pytest module against a live k8s workload, emits JUnit XML. |

## Distributed Build/RE Grid

Two independent distributed-build mechanisms exist on the cluster — they solve
different problems and do not overlap:

| Mechanism | What it distributes | Used by |
| --- | --- | --- |
| k8s scheduler (no pin) | Full privileged bootc OCI builds (needs real FUSE/mount-namespace access) | `dakota-build-pipeline`, `bluefin-server-build-pipeline` |
| Buildbarn (`buildbarn` namespace) | Lightweight BuildStream remote-execution actions (chroot-only sandbox, `CAP_SYS_CHROOT`) | `bst-qa-pipeline` only |

Buildbarn topology (2 storage shards, 1 scheduler, 2 frontend replicas, 1
worker+runner DaemonSet pair per node — one shard/worker pair pinned per node
via `podAntiAffinity`) is defined in `manifests/buildbarn-*.yaml`. It **cannot**
run the real dakota/bluefin-server OCI builds — those require privileges
Buildbarn's runner deliberately does not grant.

## Cache Warming (Pollers)

| CronWorkflow | Interval | Triggers | Keeps warm |
| --- | --- | --- | --- |
| `digest-watch` | 5 min | `build-containerdisk` (force rebuild) when `bluefin`/`bluefin-lts` GHCR digest changes | The containerDisk that `bluefin-qa-pipeline`'s `assert-cd` depends on |
| `dakota-commit-poller` | 5 min | `dakota-build-pipeline` when `dakota:testing` gets a new commit digest | `bst-artifact-server` cache for the dakota-specific BuildStream layer |
| `image-poll-bluefin-{testing,stable}` / `image-poll-lts-{testing,stable}` | 10 min | `bluefin-qa-pipeline` when the respective GHCR tag digest changes | Test coverage freshness (not a build cache) |
| `flatcar-kernel-poller` | 10 min | `flatcar-kernel-build` when kernel.org's latest stable version changes | Flatcar kernel build cache |
| `flatcar-kernel-gate` | 30 min | (gate/promotion check, see `docs/skills/flatcar-node-onboarding.md`) | N/A |

Dakota/Cosmic/Bluefin Server/BST lanes now enforce local-only cache policy: workflows override
project cache remotes and do not push to external caches. Cold runs may fetch
from upstream source origins, but cache writes stay in-cluster (`bst-artifact-server`).

**`bluefin-server-build-pipeline` has no poller at all** — manual-trigger only,
and deliberately local-cache-only (commit `22c7d2ad`, "use local-cache path... for
extreme stability"). It is cold by design on every run; this is an intentional
trade-off, not a gap.

**`nightly-dakota` does not warm anything** — it's wired to `dakota-qa-pipeline`
(test runner against pre-built images), not `dakota-build-pipeline` (the actual
compile step). The real dakota cache-warming trigger is `dakota-commit-poller`.

All 6 bluefin/lts pollers plus `digest-watch`/`dakota-commit-poller`/
`flatcar-kernel-poller`/`flatcar-kernel-gate` were briefly suspended (2026-06-28
through 2026-07-02) while a real bug was fixed — `containerdisk-tag`'s default
value self-referenced `{{workflow.parameters.image-tag}}` inside the same
`arguments.parameters` block, which Argo does not resolve. Fixed in commit
`be045b12` with an explicit literal default plus a regression test
(`tests/unit/test_workflow_defaults.py`). Re-enabled 2026-07-03 after
confirming the fix holds and re-verifying package/network availability.

**Known gap:** `digest-watch` only rebuilds a containerDisk when the upstream
GHCR digest *changes* vs the `containerdisk-source-digests` ConfigMap — it has
no way to notice the containerDisk itself disappeared out-of-band (e.g. a Zot
disk wipe) while the upstream digest stayed the same. This actually happened
2026-07-03 after the ghost XFS migration wiped Zot: `bluefin-containerdisk`
was completely absent, `digest-watch` kept reporting "no change — skipping",
and `bluefin-qa-pipeline`'s `assert-cd` would have failed indefinitely.
Recovered by manually submitting `build-containerdisk` with `force=true`
directly. If this needs to self-heal automatically next time, `digest-watch`
would need an additional Zot-existence check (like `assert-cd`'s skopeo probe)
alongside the digest comparison — not implemented yet.

## Nightly Schedule

| CronWorkflow | Time (UTC) | Pipeline | Parameters |
| --- | --- | --- | --- |
| `nightly-smoke` | 02:00 | `bluefin-qa-pipeline` | `image=ghcr.io/projectbluefin/bluefin`, `image-tag=testing`, `containerdisk-tag=testing`, `namespace=bluefin-test`, `suites=smoke,developer,system` |
| `nightly-smoke-lts` | 02:30 | `bluefin-qa-pipeline` | `image=ghcr.io/projectbluefin/bluefin-lts`, `image-tag=testing`, `containerdisk-tag=lts-testing`, `namespace=bluefin-lts-test`, `suites=smoke,developer,system`, `vm-memory=8Gi` |
| `nightly-dakota` | — | `dakota-qa-pipeline` | Tests pre-built images only — does not build/warm cache. Currently `suspend: true`. |
| `nightly-knuckle` | 03:30 | `knuckle-qa-pipeline` | `branch=main`, `namespace=knuckle-test`, `suite=smoke`, `tests-branch=main` |

## Priority Classes

| PriorityClass | Value | Applied to |
| --- | --- | --- |
| `lab-test-vm` | 1,000,000, `PreemptLowerPriority` | All KubeVirt test VMs (`provision-bluefin-vm`, `provision-flatcar-vm`, `provision-gnomeos-vm`, `knuckle-qa-pipeline`'s two VM specs) |
| `bst-build` | (see `manifests/bst-build-priorityclass.yaml`) | Heavy/long BuildStream compiles: `dakota-build-pipeline`, `bluefin-server-build-pipeline`, `flatcar-kernel-build`'s VM spec |

Test VMs are meant to win resource contention over background build workloads —
`lab-test-vm`'s higher priority value plus `PreemptLowerPriority` enforces this
against any pod using `bst-build`.

## Resource Profiles

Pod resource requests/limits used by workflow steps:

| Template | CPU req/limit | Memory req/limit |
| --- | --- | --- |
| `build-containerdisk/check` | 100m / 500m | 128Mi / 512Mi |
| `build-containerdisk/install-to-disk` | 4 / 8 | 4Gi / 12Gi |
| `build-containerdisk/convert-and-push` | 2 / 4 | 4Gi / 8Gi |
| `wait-for-vm-ready` | 100m / 500m | 128Mi / 256Mi |
| `run-gnome-tests` | 1 / 2 | 1Gi / 2Gi |
| `dakota-build-pipeline/bst-build` | 4 / 8 | 14Gi / 28Gi |
| `cosmic-build-pipeline/bst-build` | 4 / 8 | 14Gi / 28Gi |
| `bluefin-server-build-pipeline/bst-build` | 6 / 10 | 16Gi / 30Gi |
| `knuckle build-installer` | 4 / 4 | 8Gi / 8Gi |
| `knuckle write-ignition` | 100m / 500m | 128Mi / 256Mi |
| `knuckle boot-installer` | 1 / 2 | 1Gi / 2Gi |
| `knuckle wait-install-complete` / `transition-to-installed` / `discover-installed-ip` | 250m / 1 | 256Mi / 512Mi |
| `knuckle cleanup-installer-artifacts` | 50m / 200m | 64Mi / 128Mi |
